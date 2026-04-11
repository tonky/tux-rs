// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * tuxedo_uw_fan - Uniwill EC fan control sysfs passthrough
 *
 * Exposes Uniwill EC fan duty, temperatures, and mode as sysfs attributes
 * at /sys/devices/platform/tuxedo-uw-fan/.
 *
 * Stateless — all policy (fan curves, profiles) lives in the userspace daemon.
 *
 * Primary:  ACPI methods ECRR/ECRW on \_SB.INOU
 * Fallback: WMI evaluate on ABBC0F6F GUID (Uniwill BC)
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/module.h>
#include <linux/platform_device.h>
#include <linux/acpi.h>
#include <linux/wmi.h>
#include <linux/mutex.h>
#include <linux/delay.h>

#define DRIVER_NAME		"tuxedo-uw-fan"

/* Uniwill WMI GUIDs — all six must be present for identification */
#define UW_WMI_GUID_BA		"ABBC0F6D-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_GUID_BB		"ABBC0F6E-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_GUID_BC		"ABBC0F6F-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_EVT_0		"ABBC0F70-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_EVT_1		"ABBC0F71-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_EVT_2		"ABBC0F72-8EA1-11D1-00A0-C90629100000"

/* WMI method constants */
#define UW_WMI_INSTANCE		0x00
#define UW_WMI_METHOD_ID	0x04
#define UW_WMI_FN_WRITE	0
#define UW_WMI_FN_READ		1
#define UW_WMI_ERROR		0xfefefefe

/* EC register addresses */
#define EC_FAN0_PWM		0x1804	/* Fan 1 duty 0-200 EC scale (read) */
#define EC_FAN1_PWM		0x1809	/* Fan 2 duty 0-200 EC scale (read) */
#define EC_CPU_TEMP		0x043e	/* CPU temperature °C */
#define EC_GPU_TEMP		0x044f	/* GPU temperature °C */
#define EC_MODE			0x0751	/* Mode register */
#define EC_MODE_MANUAL_BIT	6	/* Bit 6 = 0x40 = manual/full-fan */

/*
 * Universal EC fan table registers.
 * On newer Uniwill ECs (bit 6 of 0x078e = 1), the EC runs its own
 * control loop using a custom 16-zone fan table.  We configure a
 * single zone spanning 0–115°C so the speed entry at offset 0
 * acts as a flat duty target — the EC applies it automatically
 * without needing continuous PWM re-writes from userspace.
 */
#define EC_FEATS		0x078e	/* Feature flags register */
#define EC_FEATS_UNIV_FAN_BIT	6	/* Bit 6 = universal EC fan ctl */
#define EC_FEATS_CHG_PROF_BIT	3	/* Bit 3 = charging profiles supported */

#define EC_CHG_PRIO_FEATS	0x0742	/* Charging priority feature register */
#define EC_CHG_PRIO_FEATS_BIT	5	/* Bit 5 = charging priority supported */

#define EC_CHG_PROFILE		0x07a6	/* Charging profile register (bits 4-5) */
#define EC_CHG_PRIORITY		0x07cc	/* Charging priority register (bit 7) */

#define EC_CUSTOM_FAN_CFG0	0x07c5	/* Fan table config 0 */
#define EC_CUSTOM_FAN_CFG0_BIT	7	/* Separate CPU/GPU tables */
#define EC_CUSTOM_FAN_CFG1	0x07c6	/* Fan table config 1 */
#define EC_CUSTOM_FAN_CFG1_BIT	2	/* Enable 0x0fxx tables */

/* CPU fan table (16 zones) */
#define EC_CPU_FAN_END_TEMP	0x0f00	/* +0..+15: end temperatures */
#define EC_CPU_FAN_START_TEMP	0x0f10	/* +0..+15: start temperatures */
#define EC_CPU_FAN_SPEED	0x0f20	/* +0..+15: fan speeds */
/* GPU fan table */
#define EC_GPU_FAN_END_TEMP	0x0f30
#define EC_GPU_FAN_START_TEMP	0x0f40
#define EC_GPU_FAN_SPEED	0x0f50

static DEFINE_MUTEX(ec_lock);
static struct platform_device *pdev;
static acpi_handle inou_handle;
static bool use_inou;
static bool has_univ_fan;	/* universal EC custom fan table support */
static bool has_chg_profile;	/* EC charging profile support */
static bool has_chg_priority;	/* EC charging priority support */
static bool fans_initialized;	/* custom fan table has been configured */

/* ------------------------------------------------------------------ */
/* EC access: ACPI INOU path (primary)                                */
/* ------------------------------------------------------------------ */

static int __ec_read_inou(u16 addr, u8 *data)
{
	union acpi_object param;
	struct acpi_object_list input = { .count = 1, .pointer = &param };
	unsigned long long val;
	acpi_status status;

	param.type = ACPI_TYPE_INTEGER;
	param.integer.value = addr;

	status = acpi_evaluate_integer(inou_handle, "ECRR", &input, &val);
	if (ACPI_FAILURE(status))
		return -EIO;

	*data = val & 0xff;
	return 0;
}

static int __ec_write_inou(u16 addr, u8 data)
{
	union acpi_object params[2];
	struct acpi_object_list input = { .count = 2, .pointer = params };
	unsigned long long val;
	acpi_status status;

	params[0].type = ACPI_TYPE_INTEGER;
	params[0].integer.value = addr;
	params[1].type = ACPI_TYPE_INTEGER;
	params[1].integer.value = data;

	status = acpi_evaluate_integer(inou_handle, "ECRW", &input, &val);
	if (ACPI_FAILURE(status))
		return -EIO;

	return 0;
}

/* ------------------------------------------------------------------ */
/* EC access: WMI path (fallback)                                     */
/* ------------------------------------------------------------------ */

static int wmi_evaluate(u8 function, u32 arg, u32 *result)
{
	u8 inbuf[40] = {0};
	struct acpi_buffer in = { sizeof(inbuf), inbuf };
	struct acpi_buffer out = { ACPI_ALLOCATE_BUFFER, NULL };
	union acpi_object *obj;
	acpi_status status;
	int ret = 0;

	memcpy(&inbuf[0], &arg, sizeof(arg));
	inbuf[5] = function;

	status = wmi_evaluate_method(UW_WMI_GUID_BC, UW_WMI_INSTANCE,
				     UW_WMI_METHOD_ID, &in, &out);
	if (ACPI_FAILURE(status))
		return -EIO;

	obj = out.pointer;
	if (obj && obj->type == ACPI_TYPE_BUFFER && obj->buffer.length >= 4)
		memcpy(result, obj->buffer.pointer, sizeof(*result));
	else
		ret = -EIO;

	kfree(obj);
	return ret;
}

static int __ec_read_wmi(u16 addr, u8 *data)
{
	u32 result;
	int ret;

	ret = wmi_evaluate(UW_WMI_FN_READ, (u32)addr, &result);
	if (ret)
		return ret;
	if (result == UW_WMI_ERROR)
		return -EIO;

	*data = result & 0xff;
	return 0;
}

static int __ec_write_wmi(u16 addr, u8 data)
{
	u32 result;
	int ret;

	ret = wmi_evaluate(UW_WMI_FN_WRITE,
			   ((u32)data << 16) | (u32)addr, &result);
	if (ret)
		return ret;
	if (result == UW_WMI_ERROR)
		return -EIO;

	return 0;
}

/* ------------------------------------------------------------------ */
/* Unified EC access — raw (must hold ec_lock)                        */
/* ------------------------------------------------------------------ */

static int __ec_read(u16 addr, u8 *data)
{
	int ret;

	ret = use_inou ? __ec_read_inou(addr, data)
		       : __ec_read_wmi(addr, data);
	usleep_range(5000, 7000); /* 6 ms inter-operation delay */
	return ret;
}

static int __ec_write(u16 addr, u8 data)
{
	int ret;

	ret = use_inou ? __ec_write_inou(addr, data)
		       : __ec_write_wmi(addr, data);
	usleep_range(5000, 7000);
	return ret;
}

/* Public wrappers with locking */

static int uw_ec_read(u16 addr, u8 *data)
{
	int ret;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_read(addr, data);
	mutex_unlock(&ec_lock);
	return ret;
}

/* Atomic read-modify-write of a single bit */
static int ec_rmw_bit(u16 addr, u8 bit, bool set)
{
	u8 val;
	int ret;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_read(addr, &val);
	if (!ret) {
		if (set)
			val |= (1 << bit);
		else
			val &= ~(1 << bit);
		ret = __ec_write(addr, val);
	}
	mutex_unlock(&ec_lock);
	return ret;
}

/* ------------------------------------------------------------------ */
/* Universal EC fan table setup                                       */
/* ------------------------------------------------------------------ */

/*
 * Fan speed bounds (0–200 EC scale).
 * Minimum prevents the EC from entering its own "fan-off → 30% for 3 min"
 * behaviour on some models.
 */
#define UW_FAN_DUTY_MIN		20	/* ~10 % — safe idle floor */
#define UW_FAN_DUTY_MAX		200

/*
 * Initialize the EC custom fan table: one controllable zone 0–115°C
 * with dummy unreachable zones filling the rest.  Must hold ec_lock.
 *
 * Mirrors the vendor uw_init_fan() approach in tuxedo_io.
 */
static int __uw_init_fan_table(void)
{
	u8 val;
	int ret, i;
	int temp_offset;

	if (fans_initialized)
		return 0;

	/* Ensure full-fan mode is OFF — required for custom table control */
	ret = __ec_read(EC_MODE, &val);
	if (ret)
		return ret;
	if (val & (1 << EC_MODE_MANUAL_BIT)) {
		val &= ~(1 << EC_MODE_MANUAL_BIT);
		ret = __ec_write(EC_MODE, val);
		if (ret)
			return ret;
	}

	/* Enable separate CPU/GPU fan tables (bit 7 of 0x07c5) */
	ret = __ec_read(EC_CUSTOM_FAN_CFG0, &val);
	if (ret)
		return ret;
	if (!(val & (1 << EC_CUSTOM_FAN_CFG0_BIT))) {
		val |= (1 << EC_CUSTOM_FAN_CFG0_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG0, val);
		if (ret)
			return ret;
	}

	/*
	 * Zone 0: single controllable zone covering 0–115°C.
	 * Zones 1–15: dummy unreachable zones (116, 117, ... °C)
	 * with max fan speed so the EC never leaves them silent.
	 *
	 * Table data is written BEFORE enabling CFG1 — the vendor driver
	 * does the same to avoid the EC reading a partially-populated table.
	 */
	__ec_write(EC_CPU_FAN_END_TEMP, 115);
	__ec_write(EC_CPU_FAN_START_TEMP, 0);
	__ec_write(EC_CPU_FAN_SPEED, 0);
	__ec_write(EC_GPU_FAN_END_TEMP, 120);
	__ec_write(EC_GPU_FAN_START_TEMP, 0);
	__ec_write(EC_GPU_FAN_SPEED, 0);

	temp_offset = 115;
	for (i = 1; i <= 0xf; i++) {
		__ec_write(EC_CPU_FAN_END_TEMP + i, temp_offset + i + 1);
		__ec_write(EC_CPU_FAN_START_TEMP + i, temp_offset + i);
		__ec_write(EC_CPU_FAN_SPEED + i, UW_FAN_DUTY_MAX);

		__ec_write(EC_GPU_FAN_END_TEMP + i, temp_offset + i + 1);
		__ec_write(EC_GPU_FAN_START_TEMP + i, temp_offset + i);
		__ec_write(EC_GPU_FAN_SPEED + i, UW_FAN_DUTY_MAX);
	}

	/* Enable custom fan tables (bit 2 of 0x07c6) AFTER data is written */
	ret = __ec_read(EC_CUSTOM_FAN_CFG1, &val);
	if (ret)
		return ret;
	if (!(val & (1 << EC_CUSTOM_FAN_CFG1_BIT))) {
		val |= (1 << EC_CUSTOM_FAN_CFG1_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG1, val);
		if (ret)
			return ret;
	}

	fans_initialized = true;
	pr_info("custom fan table initialized\n");
	return 0;
}

/*
 * Restore EC auto fan control: disable custom tables and clear
 * full-fan mode.  Must hold ec_lock.
 */
static int __uw_restore_auto(void)
{
	u8 val;
	int ret;

	/* Disable custom fan tables */
	ret = __ec_read(EC_CUSTOM_FAN_CFG1, &val);
	if (!ret && (val & (1 << EC_CUSTOM_FAN_CFG1_BIT))) {
		val &= ~(1 << EC_CUSTOM_FAN_CFG1_BIT);
		__ec_write(EC_CUSTOM_FAN_CFG1, val);
	}

	/* Clear full-fan / manual mode bit */
	ret = __ec_read(EC_MODE, &val);
	if (!ret && (val & (1 << EC_MODE_MANUAL_BIT))) {
		val &= ~(1 << EC_MODE_MANUAL_BIT);
		__ec_write(EC_MODE, val);
	}

	fans_initialized = false;
	return 0;
}

/* ------------------------------------------------------------------ */
/* sysfs attributes                                                   */
/* ------------------------------------------------------------------ */

static ssize_t fan0_pwm_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	u8 val;
	int ret = uw_ec_read(EC_FAN0_PWM, &val);

	return ret ? ret : sysfs_emit(buf, "%u\n", val);
}

static ssize_t fan0_pwm_store(struct device *dev,
			      struct device_attribute *attr,
			      const char *buf, size_t count)
{
	u8 val;
	int ret = kstrtou8(buf, 0, &val);

	if (ret)
		return ret;
	if (val == 0)
		return -EINVAL;
	val = clamp_val(val, UW_FAN_DUTY_MIN, UW_FAN_DUTY_MAX);

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;

	if (has_univ_fan) {
		ret = __uw_init_fan_table();
		if (!ret)
			ret = __ec_write(EC_CPU_FAN_SPEED, val);
		/* Also write direct PWM register for immediate effect */
		if (!ret)
			ret = __ec_write(EC_FAN0_PWM, val);
	} else {
		ret = __ec_write(EC_FAN0_PWM, val);
	}

	mutex_unlock(&ec_lock);
	return ret ? ret : count;
}

static ssize_t fan1_pwm_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	u8 val;
	int ret = uw_ec_read(EC_FAN1_PWM, &val);

	return ret ? ret : sysfs_emit(buf, "%u\n", val);
}

static ssize_t fan1_pwm_store(struct device *dev,
			      struct device_attribute *attr,
			      const char *buf, size_t count)
{
	u8 val;
	int ret = kstrtou8(buf, 0, &val);

	if (ret)
		return ret;
	if (val == 0)
		return -EINVAL;
	val = clamp_val(val, UW_FAN_DUTY_MIN, UW_FAN_DUTY_MAX);

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;

	if (has_univ_fan) {
		ret = __uw_init_fan_table();
		if (!ret)
			ret = __ec_write(EC_GPU_FAN_SPEED, val);
		/* Also write direct PWM register for immediate effect */
		if (!ret)
			ret = __ec_write(EC_FAN1_PWM, val);
	} else {
		ret = __ec_write(EC_FAN1_PWM, val);
	}

	mutex_unlock(&ec_lock);
	return ret ? ret : count;
}

static ssize_t fan_mode_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	/*
	 * Report 1 (manual) when custom fan table is active,
	 * 0 (auto) when the EC's default control is in use.
	 */
	if (has_univ_fan)
		return sysfs_emit(buf, "%u\n", fans_initialized ? 1 : 0);

	/* Legacy: read manual bit directly */
	{
		u8 val;
		int ret = uw_ec_read(EC_MODE, &val);
		if (ret)
			return ret;
		return sysfs_emit(buf, "%u\n",
				  (val >> EC_MODE_MANUAL_BIT) & 1);
	}
}

static ssize_t fan_mode_store(struct device *dev,
			      struct device_attribute *attr,
			      const char *buf, size_t count)
{
	u8 mode;
	int ret;

	ret = kstrtou8(buf, 0, &mode);
	if (ret)
		return ret;
	if (mode > 1)
		return -EINVAL;

	if (has_univ_fan) {
		if (mutex_lock_interruptible(&ec_lock))
			return -ERESTARTSYS;
		if (mode == 0)
			ret = __uw_restore_auto();
		/* mode == 1 is a no-op; init happens on first PWM write */
		mutex_unlock(&ec_lock);
	} else {
		ret = ec_rmw_bit(EC_MODE, EC_MODE_MANUAL_BIT, mode);
	}

	return ret ? ret : count;
}

static ssize_t cpu_temp_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	u8 val;
	int ret = uw_ec_read(EC_CPU_TEMP, &val);

	return ret ? ret : sysfs_emit(buf, "%u\n", val);
}

static ssize_t gpu_temp_show(struct device *dev,
			     struct device_attribute *attr, char *buf)
{
	u8 val;
	int ret = uw_ec_read(EC_GPU_TEMP, &val);

	return ret ? ret : sysfs_emit(buf, "%u\n", val);
}

static ssize_t fan_count_show(struct device *dev,
			      struct device_attribute *attr, char *buf)
{
	return sysfs_emit(buf, "2\n");
}

static DEVICE_ATTR_RW(fan0_pwm);
static DEVICE_ATTR_RW(fan1_pwm);
static DEVICE_ATTR_RW(fan_mode);
static DEVICE_ATTR_RO(cpu_temp);
static DEVICE_ATTR_RO(gpu_temp);
static DEVICE_ATTR_RO(fan_count);

/*
 * fan_table: program the full 16-zone EC fan table in one shot.
 *
 * Write format: up to 16 pairs of "end_temp speed" (space or newline
 * separated).  Temperatures in °C, speeds in EC scale (0–200).
 * Zone 0 starts at 0°C; subsequent zones start where the previous
 * zone ended.  Unused zones are filled with unreachable dummy entries
 * at max speed.  Both CPU and GPU tables are programmed identically
 * (on models without dGPU the GPU table just keeps fan1 in sync).
 *
 * Example: "45 40 65 90 80 150 100 200"
 *   → Zone 0:  0–45°C  speed=40
 *   → Zone 1: 45–65°C  speed=90
 *   → Zone 2: 65–80°C  speed=150
 *   → Zone 3: 80–100°C speed=200
 *   → Zones 4–15: dummy at max speed
 *
 * Show: display the current table zones.
 */
static ssize_t fan_table_show(struct device *dev,
			      struct device_attribute *attr, char *buf)
{
	u8 end_temp, start_temp, speed;
	int ret, i, len = 0;

	if (!has_univ_fan)
		return -ENODEV;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;

	for (i = 0; i < 16; i++) {
		ret = __ec_read(EC_CPU_FAN_START_TEMP + i, &start_temp);
		if (ret)
			goto out;
		ret = __ec_read(EC_CPU_FAN_END_TEMP + i, &end_temp);
		if (ret)
			goto out;
		ret = __ec_read(EC_CPU_FAN_SPEED + i, &speed);
		if (ret)
			goto out;

		len += sysfs_emit_at(buf, len, "%u %u %u\n",
				     start_temp, end_temp, speed);
	}

out:
	mutex_unlock(&ec_lock);
	return ret ? ret : len;
}

static ssize_t fan_table_store(struct device *dev,
			       struct device_attribute *attr,
			       const char *buf, size_t count)
{
	u8 end_temps[16], speeds[16];
	int n_zones = 0;
	int ret, i;
	const char *p = buf;
	u8 val;

	if (!has_univ_fan)
		return -ENODEV;

	/* Parse pairs of "end_temp speed" */
	while (n_zones < 16 && *p) {
		unsigned int t, s;
		int chars;

		/* Skip whitespace */
		while (*p == ' ' || *p == '\t' || *p == '\n')
			p++;
		if (!*p)
			break;

		if (sscanf(p, "%u %u%n", &t, &s, &chars) < 2)
			return -EINVAL;
		p += chars;

		if (t > 130 || s > UW_FAN_DUTY_MAX)
			return -EINVAL;

		end_temps[n_zones] = (u8)t;
		speeds[n_zones] = clamp_val((u8)s, UW_FAN_DUTY_MIN,
					    UW_FAN_DUTY_MAX);
		n_zones++;
	}

	if (n_zones == 0)
		return -EINVAL;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;

	/* Disable custom tables while updating */
	ret = __ec_read(EC_CUSTOM_FAN_CFG1, &val);
	if (ret)
		goto out;
	if (val & (1 << EC_CUSTOM_FAN_CFG1_BIT)) {
		val &= ~(1 << EC_CUSTOM_FAN_CFG1_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG1, val);
		if (ret)
			goto out;
	}

	/* Ensure full-fan mode is OFF */
	ret = __ec_read(EC_MODE, &val);
	if (ret)
		goto out;
	if (val & (1 << EC_MODE_MANUAL_BIT)) {
		val &= ~(1 << EC_MODE_MANUAL_BIT);
		ret = __ec_write(EC_MODE, val);
		if (ret)
			goto out;
	}

	/* Enable separate CPU/GPU fan tables */
	ret = __ec_read(EC_CUSTOM_FAN_CFG0, &val);
	if (ret)
		goto out;
	if (!(val & (1 << EC_CUSTOM_FAN_CFG0_BIT))) {
		val |= (1 << EC_CUSTOM_FAN_CFG0_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG0, val);
		if (ret)
			goto out;
	}

	/* Write user-supplied zones to both CPU and GPU tables */
	for (i = 0; i < n_zones; i++) {
		u8 start = (i == 0) ? 0 : end_temps[i - 1];

		/* CPU table */
		ret = __ec_write(EC_CPU_FAN_END_TEMP + i, end_temps[i]);
		if (ret)
			goto out;
		ret = __ec_write(EC_CPU_FAN_START_TEMP + i, start);
		if (ret)
			goto out;
		ret = __ec_write(EC_CPU_FAN_SPEED + i, speeds[i]);
		if (ret)
			goto out;

		/* GPU table (identical) */
		ret = __ec_write(EC_GPU_FAN_END_TEMP + i, end_temps[i]);
		if (ret)
			goto out;
		ret = __ec_write(EC_GPU_FAN_START_TEMP + i, start);
		if (ret)
			goto out;
		ret = __ec_write(EC_GPU_FAN_SPEED + i, speeds[i]);
		if (ret)
			goto out;
	}

	/* Fill remaining zones with unreachable dummies at max speed */
	{
		u8 base = (n_zones > 0) ? end_temps[n_zones - 1] : 115;

		for (i = n_zones; i < 16; i++) {
			u8 s = base + (i - n_zones);
			u8 e = s + 1;

			__ec_write(EC_CPU_FAN_END_TEMP + i, e);
			__ec_write(EC_CPU_FAN_START_TEMP + i, s);
			__ec_write(EC_CPU_FAN_SPEED + i, UW_FAN_DUTY_MAX);

			__ec_write(EC_GPU_FAN_END_TEMP + i, e);
			__ec_write(EC_GPU_FAN_START_TEMP + i, s);
			__ec_write(EC_GPU_FAN_SPEED + i, UW_FAN_DUTY_MAX);
		}
	}

	/* Re-enable custom fan tables */
	ret = __ec_read(EC_CUSTOM_FAN_CFG1, &val);
	if (!ret) {
		val |= (1 << EC_CUSTOM_FAN_CFG1_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG1, val);
	}

	/*
	 * Poke the direct PWM registers so the EC applies the new table
	 * immediately.  Without this the EC keeps running at the previous
	 * duty until its own periodic re-evaluation kicks in (seconds).
	 *
	 * Read current CPU temp, find matching zone, write that zone's
	 * speed to both fan PWM registers.
	 */
	if (!ret) {
		u8 temp, duty = speeds[0];

		if (!__ec_read(EC_CPU_TEMP, &temp)) {
			for (i = 0; i < n_zones; i++) {
				u8 start = (i == 0) ? 0 : end_temps[i - 1];

				if (temp >= start && temp <= end_temps[i]) {
					duty = speeds[i];
					break;
				}
			}
		}
		__ec_write(EC_FAN0_PWM, duty);
		__ec_write(EC_FAN1_PWM, duty);
	}

	fans_initialized = true;
	pr_info("fan table programmed with %d zones\n", n_zones);

out:
	mutex_unlock(&ec_lock);
	return ret ? ret : count;
}

static DEVICE_ATTR_RW(fan_table);

/* ------------------------------------------------------------------ */
/* Charging profile/priority sysfs attributes                        */
/* ------------------------------------------------------------------ */

static const char * const chg_profile_names[] = {
	"high_capacity", "balanced", "stationary"
};

static const char * const chg_priority_names[] = {
	"charge", "performance"
};

static ssize_t charge_profile_show(struct device *dev,
				   struct device_attribute *attr, char *buf)
{
	u8 val;
	int ret;

	if (!has_chg_profile)
		return -ENODEV;

	ret = uw_ec_read(EC_CHG_PROFILE, &val);
	if (ret)
		return ret;

	val = (val >> 4) & 0x03;
	if (val >= ARRAY_SIZE(chg_profile_names))
		return sysfs_emit(buf, "unknown\n");

	return sysfs_emit(buf, "%s\n", chg_profile_names[val]);
}

static ssize_t charge_profile_store(struct device *dev,
				    struct device_attribute *attr,
				    const char *buf, size_t count)
{
	u8 val, reg;
	int ret, i;

	if (!has_chg_profile)
		return -ENODEV;

	/* Match input to a known profile name */
	for (i = 0; i < ARRAY_SIZE(chg_profile_names); i++) {
		if (sysfs_streq(buf, chg_profile_names[i]))
			break;
	}
	if (i >= ARRAY_SIZE(chg_profile_names))
		return -EINVAL;

	val = (u8)i;

	/* Read-modify-write bits 4-5 */
	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_read(EC_CHG_PROFILE, &reg);
	if (!ret) {
		reg = (reg & ~(0x03 << 4)) | (val << 4);
		ret = __ec_write(EC_CHG_PROFILE, reg);
	}
	mutex_unlock(&ec_lock);
	return ret ? ret : count;
}

static ssize_t charge_priority_show(struct device *dev,
				    struct device_attribute *attr, char *buf)
{
	u8 val;
	int ret;

	if (!has_chg_priority)
		return -ENODEV;

	ret = uw_ec_read(EC_CHG_PRIORITY, &val);
	if (ret)
		return ret;

	val = (val >> 7) & 0x01;
	if (val >= ARRAY_SIZE(chg_priority_names))
		return sysfs_emit(buf, "unknown\n");

	return sysfs_emit(buf, "%s\n", chg_priority_names[val]);
}

static ssize_t charge_priority_store(struct device *dev,
				     struct device_attribute *attr,
				     const char *buf, size_t count)
{
	u8 val, reg;
	int ret, i;

	if (!has_chg_priority)
		return -ENODEV;

	for (i = 0; i < ARRAY_SIZE(chg_priority_names); i++) {
		if (sysfs_streq(buf, chg_priority_names[i]))
			break;
	}
	if (i >= ARRAY_SIZE(chg_priority_names))
		return -EINVAL;

	val = (u8)i;

	/* Read-modify-write bit 7 */
	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_read(EC_CHG_PRIORITY, &reg);
	if (!ret) {
		reg = (reg & ~(1 << 7)) | (val << 7);
		ret = __ec_write(EC_CHG_PRIORITY, reg);
	}
	mutex_unlock(&ec_lock);
	return ret ? ret : count;
}

static DEVICE_ATTR_RW(charge_profile);
static DEVICE_ATTR_RW(charge_priority);

static struct attribute *uw_fan_attrs[] = {
	&dev_attr_fan0_pwm.attr,
	&dev_attr_fan1_pwm.attr,
	&dev_attr_fan_mode.attr,
	&dev_attr_cpu_temp.attr,
	&dev_attr_gpu_temp.attr,
	&dev_attr_fan_count.attr,
	&dev_attr_fan_table.attr,
	&dev_attr_charge_profile.attr,
	&dev_attr_charge_priority.attr,
	NULL,
};

static umode_t uw_attr_visible(struct kobject *kobj,
			       struct attribute *attr, int n)
{
	if (attr == &dev_attr_charge_profile.attr && !has_chg_profile)
		return 0;
	if (attr == &dev_attr_charge_priority.attr && !has_chg_priority)
		return 0;
	return attr->mode;
}

static const struct attribute_group uw_fan_group = {
	.attrs = uw_fan_attrs,
	.is_visible = uw_attr_visible,
};

/* ------------------------------------------------------------------ */
/* Module init / exit                                                 */
/* ------------------------------------------------------------------ */

static int __init uw_fan_init(void)
{
	acpi_status status;
	int ret;

	/* Primary: WMI (proven path — matches vendor driver) */
	if (wmi_has_guid(UW_WMI_GUID_BA) &&
	    wmi_has_guid(UW_WMI_GUID_BB) &&
	    wmi_has_guid(UW_WMI_GUID_BC) &&
	    wmi_has_guid(UW_WMI_EVT_0)  &&
	    wmi_has_guid(UW_WMI_EVT_1)  &&
	    wmi_has_guid(UW_WMI_EVT_2)) {
		use_inou = false;
	} else {
		/* Fallback: ACPI INOU interface */
		status = acpi_get_handle(NULL, "\\_SB.INOU", &inou_handle);
		if (ACPI_FAILURE(status)) {
			pr_debug("no Uniwill interface found\n");
			return -ENODEV;
		}
		use_inou = true;
	}

	/* Detect universal EC fan control (bit 6 of 0x078e) */
	{
		u8 feats;

		has_univ_fan = false;
		has_chg_profile = false;
		if (!uw_ec_read(EC_FEATS, &feats)) {
			has_univ_fan = (feats >> EC_FEATS_UNIV_FAN_BIT) & 1;
			has_chg_profile = (feats >> EC_FEATS_CHG_PROF_BIT) & 1;
		}
	}

	/* Detect charging priority support (bit 5 of 0x0742) */
	{
		u8 feats;

		has_chg_priority = false;
		if (!uw_ec_read(EC_CHG_PRIO_FEATS, &feats))
			has_chg_priority = (feats >> EC_CHG_PRIO_FEATS_BIT) & 1;
	}

	/*
	 * EC initialisation — matches vendor uniwill_keyboard_probe():
	 *  - 0x0751 = 0x00 : set balanced performance profile
	 *  - 0x0741 = 0x01 : enable manual / custom mode
	 * Without these the EC silently ignores charging-profile writes.
	 */
	mutex_lock(&ec_lock);
	__ec_write(0x0751, 0x00);
	__ec_write(0x0741, 0x01);
	mutex_unlock(&ec_lock);

	pdev = platform_device_register_simple(DRIVER_NAME, -1, NULL, 0);
	if (IS_ERR(pdev))
		return PTR_ERR(pdev);

	ret = sysfs_create_group(&pdev->dev.kobj, &uw_fan_group);
	if (ret) {
		platform_device_unregister(pdev);
		return ret;
	}

	pr_info("initialized (%s interface, %s fan control, charging: profile=%s priority=%s)\n",
		use_inou ? "INOU" : "WMI",
		has_univ_fan ? "universal" : "legacy",
		has_chg_profile ? "yes" : "no",
		has_chg_priority ? "yes" : "no");
	return 0;
}

static void __exit uw_fan_exit(void)
{
	/* Restore EC auto control before unloading */
	if (has_univ_fan && fans_initialized) {
		mutex_lock(&ec_lock);
		__uw_restore_auto();
		mutex_unlock(&ec_lock);
	}

	sysfs_remove_group(&pdev->dev.kobj, &uw_fan_group);
	platform_device_unregister(pdev);
	pr_info("removed\n");
}

module_init(uw_fan_init);
module_exit(uw_fan_exit);

MODULE_AUTHOR("TUXEDO Computers GmbH <tux@tuxedocomputers.com>");
MODULE_DESCRIPTION("Uniwill EC fan control sysfs passthrough");
MODULE_LICENSE("GPL");
MODULE_ALIAS("wmi:" UW_WMI_GUID_BC);
