// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * tuxedo_nb04 - NB04 WMI BS sysfs passthrough
 *
 * Exposes NB04 sensors and power profile as sysfs attributes at
 * /sys/devices/platform/tuxedo-nb04/.
 *
 * WMI BS GUID: 1F174999-3A4E-4311-900D-7BE7166D5055
 * 8-byte input buffer, 80-byte output buffer.
 * Status word at out[0..1] LE must equal 0 for success.
 *
 * No direct fan PWM control — fans are managed by power profile selection.
 *
 * Stateless — all policy lives in the userspace daemon.
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/module.h>
#include <linux/platform_device.h>
#include <linux/wmi.h>
#include <linux/mutex.h>

#define DRIVER_NAME		"tuxedo-nb04"
#define NB04_WMI_BS_GUID	"1F174999-3A4E-4311-900D-7BE7166D5055"

#define BS_INPUT_LEN		8
#define BS_OUTPUT_LEN		80

/* WMI method IDs */
#define WMI_METHOD_FANS		0x02
#define WMI_METHOD_CPU		0x04
#define WMI_METHOD_GPU		0x06
#define WMI_METHOD_PROFILE	0x07

/* Power profile values */
#define PROFILE_BATTERY		0
#define PROFILE_BALANCED	1
#define PROFILE_PERFORMANCE	2
#define PROFILE_MAX		3

static DEFINE_MUTEX(nb04_lock);
static struct platform_device *pdev;

/* ------------------------------------------------------------------ */
/* WMI call helper                                                    */
/* ------------------------------------------------------------------ */

static int nb04_wmi_call(u32 method_id, u8 *in, u8 *out)
{
	struct acpi_buffer acpi_in = { BS_INPUT_LEN, in };
	struct acpi_buffer acpi_out = { ACPI_ALLOCATE_BUFFER, NULL };
	union acpi_object *obj;
	acpi_status status;
	u16 wmi_return;
	int ret = 0;

	if (mutex_lock_interruptible(&nb04_lock))
		return -ERESTARTSYS;
	status = wmi_evaluate_method(NB04_WMI_BS_GUID, 0, method_id,
				     &acpi_in, &acpi_out);
	mutex_unlock(&nb04_lock);

	if (ACPI_FAILURE(status))
		return -EIO;

	obj = acpi_out.pointer;
	if (!obj)
		return -ENODATA;

	if (obj->type != ACPI_TYPE_BUFFER ||
	    obj->buffer.length != BS_OUTPUT_LEN) {
		kfree(obj);
		return -EIO;
	}

	memcpy(out, obj->buffer.pointer, BS_OUTPUT_LEN);
	kfree(obj);

	/* Check WMI return status (LE u16 at out[0..1]) */
	wmi_return = (out[1] << 8) | out[0];
	if (wmi_return != 0)
		ret = -EIO;

	return ret;
}

/* ------------------------------------------------------------------ */
/* sysfs attributes                                                   */
/* ------------------------------------------------------------------ */

/* cpu_temp — CPU temperature in °C (RO) */
static ssize_t cpu_temp_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	u8 in[BS_INPUT_LEN] = {0};
	u8 out[BS_OUTPUT_LEN] = {0};
	int ret;

	ret = nb04_wmi_call(WMI_METHOD_CPU, in, out);
	if (ret)
		return ret;

	return sysfs_emit(buf, "%u\n", out[2]);
}

/* gpu_temp — GPU temperature in °C (RO) */
static ssize_t gpu_temp_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	u8 in[BS_INPUT_LEN] = {0};
	u8 out[BS_OUTPUT_LEN] = {0};
	int ret;

	ret = nb04_wmi_call(WMI_METHOD_GPU, in, out);
	if (ret)
		return ret;

	return sysfs_emit(buf, "%u\n", out[2]);
}

/* fan0_rpm — Fan 1 RPM (RO) */
static ssize_t fan0_rpm_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	u8 in[BS_INPUT_LEN] = {0};
	u8 out[BS_OUTPUT_LEN] = {0};
	u16 rpm;
	int ret;

	ret = nb04_wmi_call(WMI_METHOD_FANS, in, out);
	if (ret)
		return ret;

	rpm = (out[3] << 8) | out[2];
	return sysfs_emit(buf, "%u\n", rpm);
}

/* fan1_rpm — Fan 2 RPM (RO) */
static ssize_t fan1_rpm_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	u8 in[BS_INPUT_LEN] = {0};
	u8 out[BS_OUTPUT_LEN] = {0};
	u16 rpm;
	int ret;

	ret = nb04_wmi_call(WMI_METHOD_FANS, in, out);
	if (ret)
		return ret;

	rpm = (out[5] << 8) | out[4];
	return sysfs_emit(buf, "%u\n", rpm);
}

/* power_profile — 0=battery, 1=balanced, 2=performance (RW) */
static ssize_t power_profile_show(struct device *dev,
				  struct device_attribute *attr, char *buf)
{
	/*
	 * The WMI BS interface only provides a set method (0x07), no get.
	 * The firmware doesn't expose the current mode via WMI BS.
	 * We return -EOPNOTSUPP; the daemon tracks the active profile.
	 */
	return -EOPNOTSUPP;
}

static ssize_t power_profile_store(struct device *dev,
				   struct device_attribute *attr,
				   const char *buf, size_t count)
{
	u8 in[BS_INPUT_LEN] = {0};
	u8 out[BS_OUTPUT_LEN] = {0};
	u8 profile;
	int ret;

	ret = kstrtou8(buf, 0, &profile);
	if (ret)
		return ret;
	if (profile >= PROFILE_MAX)
		return -EINVAL;

	in[0] = profile;

	ret = nb04_wmi_call(WMI_METHOD_PROFILE, in, out);
	if (ret)
		return ret;

	return count;
}

static DEVICE_ATTR_RO(cpu_temp);
static DEVICE_ATTR_RO(gpu_temp);
static DEVICE_ATTR_RO(fan0_rpm);
static DEVICE_ATTR_RO(fan1_rpm);
static DEVICE_ATTR_RW(power_profile);

static struct attribute *nb04_attrs[] = {
	&dev_attr_cpu_temp.attr,
	&dev_attr_gpu_temp.attr,
	&dev_attr_fan0_rpm.attr,
	&dev_attr_fan1_rpm.attr,
	&dev_attr_power_profile.attr,
	NULL,
};

static const struct attribute_group nb04_group = {
	.attrs = nb04_attrs,
};

/* ------------------------------------------------------------------ */
/* Module init / exit                                                 */
/* ------------------------------------------------------------------ */

static int __init nb04_init(void)
{
	int ret;

	if (!wmi_has_guid(NB04_WMI_BS_GUID)) {
		pr_debug("NB04 WMI BS GUID not found\n");
		return -ENODEV;
	}

	pdev = platform_device_register_simple(DRIVER_NAME, -1, NULL, 0);
	if (IS_ERR(pdev))
		return PTR_ERR(pdev);

	ret = sysfs_create_group(&pdev->dev.kobj, &nb04_group);
	if (ret) {
		platform_device_unregister(pdev);
		return ret;
	}

	pr_info("initialized, sysfs at /sys/devices/platform/%s/\n",
		DRIVER_NAME);
	return 0;
}

static void __exit nb04_exit(void)
{
	sysfs_remove_group(&pdev->dev.kobj, &nb04_group);
	platform_device_unregister(pdev);
	pr_info("removed\n");
}

module_init(nb04_init);
module_exit(nb04_exit);

MODULE_AUTHOR("TUXEDO Computers GmbH <tux@tuxedocomputers.com>");
MODULE_DESCRIPTION("NB04 WMI BS sysfs passthrough");
MODULE_LICENSE("GPL");
MODULE_ALIAS("wmi:" NB04_WMI_BS_GUID);
