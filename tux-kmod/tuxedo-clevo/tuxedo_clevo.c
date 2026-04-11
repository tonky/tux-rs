// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * tuxedo_clevo - Clevo WMI/ACPI DSM fan control sysfs passthrough
 *
 * Exposes Clevo fan info, speed control, and auto-mode as sysfs attributes
 * at /sys/devices/platform/tuxedo-clevo/.
 *
 * Primary:  WMI (GUID ABBC0F6D)
 * Fallback: ACPI DSM (device HID CLV0001)
 *
 * Stateless — all policy lives in the userspace daemon.
 * FANINFO u32 packing (parsed in userspace):
 *   duty = info & 0xFF, temp = (info >> 8) & 0xFF, rpm = (info >> 16) & 0xFFFF
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/module.h>
#include <linux/platform_device.h>
#include <linux/acpi.h>
#include <linux/wmi.h>
#include <linux/mutex.h>
#include <linux/delay.h>
#include <linux/uuid.h>

#define DRIVER_NAME		"tuxedo-clevo"

/* WMI GUIDs */
#define CLEVO_WMI_EVENT_GUID	"ABBC0F6B-8EA1-11D1-00A0-C90629100000"
#define CLEVO_WMI_METHOD_GUID	"ABBC0F6D-8EA1-11D1-00A0-C90629100000"

/* ACPI DSM */
#define CLEVO_ACPI_HID		"CLV0001"
#define CLEVO_DSM_UUID		"93f224e4-fbdc-4bbf-add6-db71bdc0afad"

/* Clevo commands */
#define CMD_GET_FANINFO1	0x63
#define CMD_GET_FANINFO2	0x64
#define CMD_GET_FANINFO3	0x6e
#define CMD_SET_FANSPEED	0x68
#define CMD_SET_FANAUTO		0x69
#define CMD_GET_BIOS_FEAT1	0x52

static DEFINE_MUTEX(clevo_lock);
static struct platform_device *pdev;

/* Transport selection */
static bool use_wmi;
static acpi_handle dsm_handle;
static guid_t dsm_guid;

/* ------------------------------------------------------------------ */
/* WMI transport                                                      */
/* ------------------------------------------------------------------ */

static int clevo_wmi_call(u8 cmd, u32 arg, u32 *result)
{
	struct acpi_buffer in = { sizeof(arg), &arg };
	struct acpi_buffer out = { ACPI_ALLOCATE_BUFFER, NULL };
	union acpi_object *obj;
	acpi_status status;
	int ret = 0;

	status = wmi_evaluate_method(CLEVO_WMI_METHOD_GUID, 0x00,
				     cmd, &in, &out);
	if (ACPI_FAILURE(status))
		return -EIO;

	obj = out.pointer;
	if (obj && obj->type == ACPI_TYPE_INTEGER) {
		if (result)
			*result = (u32)obj->integer.value;
	} else {
		ret = -EIO;
	}

	kfree(obj);
	return ret;
}

/* ------------------------------------------------------------------ */
/* ACPI DSM transport (fallback)                                      */
/* ------------------------------------------------------------------ */

static int clevo_dsm_call(u8 cmd, u32 arg, u32 *result)
{
	union acpi_object argv_data = {
		.integer.type = ACPI_TYPE_INTEGER,
		.integer.value = arg,
	};
	union acpi_object argv = {
		.package.type = ACPI_TYPE_PACKAGE,
		.package.count = 1,
		.package.elements = &argv_data,
	};
	union acpi_object *obj;

	obj = acpi_evaluate_dsm(dsm_handle, &dsm_guid, 0, cmd, &argv);
	if (!obj)
		return -EIO;

	if (obj->type == ACPI_TYPE_INTEGER) {
		if (result)
			*result = (u32)obj->integer.value;
	} else {
		ACPI_FREE(obj);
		return -EIO;
	}

	ACPI_FREE(obj);
	return 0;
}

/* ------------------------------------------------------------------ */
/* Unified command dispatch                                           */
/* ------------------------------------------------------------------ */

static int clevo_cmd(u8 cmd, u32 arg, u32 *result)
{
	int ret;

	if (mutex_lock_interruptible(&clevo_lock))
		return -ERESTARTSYS;
	ret = use_wmi ? clevo_wmi_call(cmd, arg, result)
		      : clevo_dsm_call(cmd, arg, result);
	mutex_unlock(&clevo_lock);

	return ret;
}

/* ------------------------------------------------------------------ */
/* sysfs attributes                                                   */
/* ------------------------------------------------------------------ */

/* fan0_info — Fan 1: bits[7:0]=duty, [15:8]=temp°C, [31:16]=RPM */
static ssize_t fan0_info_show(struct device *dev,
			      struct device_attribute *attr, char *buf)
{
	u32 val;
	int ret = clevo_cmd(CMD_GET_FANINFO1, 0, &val);

	return ret ? ret : sysfs_emit(buf, "%u\n", val);
}

/* fan1_info — Fan 2: same encoding */
static ssize_t fan1_info_show(struct device *dev,
			      struct device_attribute *attr, char *buf)
{
	u32 val;
	int ret = clevo_cmd(CMD_GET_FANINFO2, 0, &val);

	return ret ? ret : sysfs_emit(buf, "%u\n", val);
}

/* fan2_info — Fan 3: same encoding (if present) */
static ssize_t fan2_info_show(struct device *dev,
			      struct device_attribute *attr, char *buf)
{
	u32 val;
	int ret = clevo_cmd(CMD_GET_FANINFO3, 0, &val);

	return ret ? ret : sysfs_emit(buf, "%u\n", val);
}

/*
 * fan_speed — Packed: 3 fan duties in one write (WO)
 *
 * u32 packing: fan0 = bits[7:0], fan1 = bits[15:8], fan2 = bits[23:16].
 * Each byte is a duty value 0–255.  A minimum floor is enforced per fan
 * to prevent accidental thermal runaway if a non-zero duty is requested.
 * Writing 0 for a fan slot is allowed (means "fan not present / ignore").
 */
#define CLEVO_FAN_DUTY_MIN	25	/* ~10 % — safe idle floor */
#define CLEVO_FAN_DUTY_MAX	255

static inline u8 clamp_duty(u8 raw)
{
	if (raw == 0)
		return 0;		/* slot unused — pass through */
	return clamp_val(raw, CLEVO_FAN_DUTY_MIN, CLEVO_FAN_DUTY_MAX);
}

static ssize_t fan_speed_store(struct device *dev,
			       struct device_attribute *attr,
			       const char *buf, size_t count)
{
	u32 val;
	u8 d0, d1, d2;
	int ret;

	ret = kstrtou32(buf, 0, &val);
	if (ret)
		return ret;

	/* Reject if any reserved upper bits [31:24] are set */
	if (val & 0xFF000000)
		return -EINVAL;

	/* Clamp each fan duty byte to the safe range */
	d0 = clamp_duty(val & 0xFF);
	d1 = clamp_duty((val >> 8) & 0xFF);
	d2 = clamp_duty((val >> 16) & 0xFF);
	val = (u32)d0 | ((u32)d1 << 8) | ((u32)d2 << 16);

	ret = clevo_cmd(CMD_SET_FANSPEED, val, NULL);
	if (ret)
		return ret;

	/* 100ms delay after speed write (firmware processing time) */
	msleep(100);

	return count;
}

/* fan_auto — Restore hardware auto mode (WO trigger) */
static ssize_t fan_auto_store(struct device *dev,
			      struct device_attribute *attr,
			      const char *buf, size_t count)
{
	int ret;

	ret = clevo_cmd(CMD_SET_FANAUTO, 0, NULL);
	return ret ? ret : count;
}

static DEVICE_ATTR_RO(fan0_info);
static DEVICE_ATTR_RO(fan1_info);
static DEVICE_ATTR_RO(fan2_info);
static DEVICE_ATTR_WO(fan_speed);
static DEVICE_ATTR_WO(fan_auto);

static struct attribute *clevo_attrs[] = {
	&dev_attr_fan0_info.attr,
	&dev_attr_fan1_info.attr,
	&dev_attr_fan2_info.attr,
	&dev_attr_fan_speed.attr,
	&dev_attr_fan_auto.attr,
	NULL,
};

static const struct attribute_group clevo_group = {
	.attrs = clevo_attrs,
};

/* ------------------------------------------------------------------ */
/* ACPI device discovery for DSM fallback                             */
/* ------------------------------------------------------------------ */

static acpi_status find_clevo_dsm(acpi_handle handle, u32 level,
				  void *ctx, void **ret)
{
	*ret = handle;
	return AE_CTRL_TERMINATE;
}

/* ------------------------------------------------------------------ */
/* Module init / exit                                                 */
/* ------------------------------------------------------------------ */

static int __init clevo_init(void)
{
	u32 bios_feat;
	int ret;

	guid_parse(CLEVO_DSM_UUID, &dsm_guid);

	/* Try WMI first */
	if (wmi_has_guid(CLEVO_WMI_EVENT_GUID) &&
	    wmi_has_guid(CLEVO_WMI_METHOD_GUID)) {
		/* Validate: cmd 0x52 must return a valid integer */
		ret = clevo_wmi_call(CMD_GET_BIOS_FEAT1, 0, &bios_feat);
		if (ret == 0 && bios_feat != 0xffffffff) {
			use_wmi = true;
			goto register_dev;
		}
	}

	/* Fallback: ACPI DSM */
	dsm_handle = NULL;
	acpi_get_devices(CLEVO_ACPI_HID, find_clevo_dsm,
			 NULL, (void **)&dsm_handle);
	if (!dsm_handle) {
		pr_debug("no Clevo WMI or ACPI interface found\n");
		return -ENODEV;
	}

	/* Validate DSM: cmd 0x52 must return a valid integer */
	ret = clevo_dsm_call(CMD_GET_BIOS_FEAT1, 0, &bios_feat);
	if (ret || bios_feat == 0xffffffff) {
		pr_debug("Clevo DSM device found but validation failed\n");
		return -ENODEV;
	}
	use_wmi = false;

register_dev:
	pdev = platform_device_register_simple(DRIVER_NAME, -1, NULL, 0);
	if (IS_ERR(pdev))
		return PTR_ERR(pdev);

	ret = sysfs_create_group(&pdev->dev.kobj, &clevo_group);
	if (ret) {
		platform_device_unregister(pdev);
		return ret;
	}

	pr_info("initialized (%s interface)\n", use_wmi ? "WMI" : "DSM");
	return 0;
}

static void __exit clevo_exit(void)
{
	sysfs_remove_group(&pdev->dev.kobj, &clevo_group);
	platform_device_unregister(pdev);
	pr_info("removed\n");
}

module_init(clevo_init);
module_exit(clevo_exit);

MODULE_AUTHOR("TUXEDO Computers GmbH <tux@tuxedocomputers.com>");
MODULE_DESCRIPTION("Clevo WMI/ACPI DSM fan control sysfs passthrough");
MODULE_LICENSE("GPL");
MODULE_ALIAS("wmi:" CLEVO_WMI_METHOD_GUID);
MODULE_ALIAS("acpi*:" CLEVO_ACPI_HID ":*");
