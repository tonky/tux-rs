// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * tuxedo_tuxi - TUXEDO TUXI (TFAN) fan control sysfs passthrough
 *
 * Exposes TUXI ACPI fan speed, mode, temperature, and RPM as sysfs
 * attributes at /sys/devices/platform/tuxedo-tuxi/.
 *
 * Stateless — all policy (fan curves, profiles) lives in the userspace daemon.
 * Daemon converts tenth-Kelvin temperatures to °C: (val - 2730) / 10
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/module.h>
#include <linux/platform_device.h>
#include <linux/acpi.h>
#include <linux/mutex.h>

#define DRIVER_NAME		"tuxedo-tuxi"
#define TUXI_ACPI_HID		"TUXI0000"

static DEFINE_MUTEX(tuxi_lock);
static struct platform_device *pdev;
static acpi_handle tfan_handle;

/* ------------------------------------------------------------------ */
/* ACPI method helper                                                 */
/* ------------------------------------------------------------------ */

/*
 * Call an ACPI method on the TFAN handle with up to 2 integer arguments.
 * Returns 0 on success, negative errno on failure.
 */
static int tfan_eval(const char *method,
		     unsigned long long *args, int nargs,
		     unsigned long long *result)
{
	union acpi_object params[2];
	struct acpi_object_list input = { .count = nargs, .pointer = params };
	unsigned long long val;
	acpi_status status;
	int i;

	if (!tfan_handle)
		return -ENODEV;

	for (i = 0; i < nargs && i < ARRAY_SIZE(params); i++) {
		params[i].type = ACPI_TYPE_INTEGER;
		params[i].integer.value = args[i];
	}

	status = acpi_evaluate_integer(tfan_handle, (char *)method,
				       nargs ? &input : NULL, &val);
	if (ACPI_FAILURE(status))
		return -EIO;

	if (result)
		*result = val;
	return 0;
}

/* ------------------------------------------------------------------ */
/* sysfs attributes                                                   */
/* ------------------------------------------------------------------ */

/*
 * Fan PWM bounds (0–255 native scale).
 * A non-zero write is clamped to at least TUXI_FAN_PWM_MIN.
 * Writing 0 is rejected — use fan_mode=0 (auto) instead.
 */
#define TUXI_FAN_PWM_MIN	25	/* ~10 % — safe idle floor */

/* fan0_pwm — Fan 1 speed (0–255 native) */
static ssize_t fan0_pwm_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	unsigned long long val, args[] = { 0 };
	int ret;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("GSPD", args, 1, &val);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", (u8)val);
}

static ssize_t fan0_pwm_store(struct device *dev,
			      struct device_attribute *attr,
			      const char *buf, size_t count)
{
	unsigned long long retval, args[2];
	u8 speed;
	int ret;

	ret = kstrtou8(buf, 0, &speed);
	if (ret)
		return ret;
	if (speed == 0)
		return -EINVAL;
	speed = max_t(u8, speed, TUXI_FAN_PWM_MIN);

	args[0] = 0;
	args[1] = speed;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("SSPD", args, 2, &retval);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	if (retval)
		return -EINVAL;
	return count;
}

/* fan1_pwm — Fan 2 speed (0–255 native) */
static ssize_t fan1_pwm_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	unsigned long long val, args[] = { 1 };
	int ret;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("GSPD", args, 1, &val);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", (u8)val);
}

static ssize_t fan1_pwm_store(struct device *dev,
			      struct device_attribute *attr,
			      const char *buf, size_t count)
{
	unsigned long long retval, args[2];
	u8 speed;
	int ret;

	ret = kstrtou8(buf, 0, &speed);
	if (ret)
		return ret;
	if (speed == 0)
		return -EINVAL;
	speed = max_t(u8, speed, TUXI_FAN_PWM_MIN);

	args[0] = 1;
	args[1] = speed;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("SSPD", args, 2, &retval);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	if (retval)
		return -EINVAL;
	return count;
}

/* fan_mode — 0=auto, 1=manual */
static ssize_t fan_mode_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	unsigned long long val;
	int ret;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("GMOD", NULL, 0, &val);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", (u8)val);
}

static ssize_t fan_mode_store(struct device *dev,
			      struct device_attribute *attr,
			      const char *buf, size_t count)
{
	unsigned long long retval, args[1];
	u8 mode;
	int ret;

	ret = kstrtou8(buf, 0, &mode);
	if (ret)
		return ret;
	if (mode > 1)
		return -EINVAL;

	args[0] = mode;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("SMOD", args, 1, &retval);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	if (retval)
		return -EINVAL;
	return count;
}

/* fan0_temp — Fan 1 temperature in tenth-Kelvin from firmware */
static ssize_t fan0_temp_show(struct device *dev,
			      struct device_attribute *attr, char *buf)
{
	unsigned long long val, args[] = { 0 };
	int ret;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("GTMP", args, 1, &val);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", (u32)val);
}

/* fan1_temp — Fan 2 temperature in tenth-Kelvin from firmware */
static ssize_t fan1_temp_show(struct device *dev,
			      struct device_attribute *attr, char *buf)
{
	unsigned long long val, args[] = { 1 };
	int ret;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("GTMP", args, 1, &val);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", (u32)val);
}

/* fan0_rpm — Fan 1 RPM */
static ssize_t fan0_rpm_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	unsigned long long val, args[] = { 0 };
	int ret;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("GRPM", args, 1, &val);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", (u16)val);
}

/* fan1_rpm — Fan 2 RPM */
static ssize_t fan1_rpm_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	unsigned long long val, args[] = { 1 };
	int ret;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("GRPM", args, 1, &val);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", (u16)val);
}

/* fan_count — number of fans from firmware */
static ssize_t fan_count_show(struct device *dev,
			      struct device_attribute *attr, char *buf)
{
	unsigned long long val;
	int ret;

	if (mutex_lock_interruptible(&tuxi_lock))
		return -ERESTARTSYS;
	ret = tfan_eval("GCNT", NULL, 0, &val);
	mutex_unlock(&tuxi_lock);

	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", (u8)val);
}

static DEVICE_ATTR_RW(fan0_pwm);
static DEVICE_ATTR_RW(fan1_pwm);
static DEVICE_ATTR_RW(fan_mode);
static DEVICE_ATTR_RO(fan0_temp);
static DEVICE_ATTR_RO(fan1_temp);
static DEVICE_ATTR_RO(fan0_rpm);
static DEVICE_ATTR_RO(fan1_rpm);
static DEVICE_ATTR_RO(fan_count);

static struct attribute *tuxi_attrs[] = {
	&dev_attr_fan0_pwm.attr,
	&dev_attr_fan1_pwm.attr,
	&dev_attr_fan_mode.attr,
	&dev_attr_fan0_temp.attr,
	&dev_attr_fan1_temp.attr,
	&dev_attr_fan0_rpm.attr,
	&dev_attr_fan1_rpm.attr,
	&dev_attr_fan_count.attr,
	NULL,
};

static const struct attribute_group tuxi_group = {
	.attrs = tuxi_attrs,
};

/* ------------------------------------------------------------------ */
/* ACPI device discovery                                              */
/* ------------------------------------------------------------------ */

static acpi_status find_tuxi_dev(acpi_handle handle, u32 level,
				 void *ctx, void **ret)
{
	*ret = handle;
	return AE_CTRL_TERMINATE;
}

/* ------------------------------------------------------------------ */
/* Module init / exit                                                 */
/* ------------------------------------------------------------------ */

static int __init tuxi_init(void)
{
	acpi_handle tuxi_handle = NULL;
	acpi_status status;
	int ret;

	/* Find TUXI0000 ACPI device */
	status = acpi_get_devices(TUXI_ACPI_HID, find_tuxi_dev,
				  NULL, (void **)&tuxi_handle);
	if (ACPI_FAILURE(status) || !tuxi_handle) {
		pr_debug("TUXI0000 ACPI device not found\n");
		return -ENODEV;
	}

	/* Find TFAN sub-device */
	status = acpi_get_handle(tuxi_handle, "TFAN", &tfan_handle);
	if (ACPI_FAILURE(status)) {
		pr_err("TFAN interface not found under TUXI0000\n");
		return -ENODEV;
	}

	pdev = platform_device_register_simple(DRIVER_NAME, -1, NULL, 0);
	if (IS_ERR(pdev))
		return PTR_ERR(pdev);

	ret = sysfs_create_group(&pdev->dev.kobj, &tuxi_group);
	if (ret) {
		platform_device_unregister(pdev);
		return ret;
	}

	pr_info("initialized\n");
	return 0;
}

static void __exit tuxi_exit(void)
{
	sysfs_remove_group(&pdev->dev.kobj, &tuxi_group);
	platform_device_unregister(pdev);
	pr_info("removed\n");
}

module_init(tuxi_init);
module_exit(tuxi_exit);

MODULE_AUTHOR("TUXEDO Computers GmbH <tux@tuxedocomputers.com>");
MODULE_DESCRIPTION("TUXEDO TUXI fan control sysfs passthrough");
MODULE_LICENSE("GPL");
MODULE_ALIAS("acpi*:" TUXI_ACPI_HID ":*");
