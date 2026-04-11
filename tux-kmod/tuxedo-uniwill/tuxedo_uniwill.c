// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * tuxedo_uniwill - Unified Uniwill platform driver
 *
 * Combines fan control, keyboard backlight (LED class), charging
 * management, battery info, fn_lock, input device, WMI events,
 * touchpad toggle, lightbar, and mini-LED local dimming into a
 * single self-contained module.
 *
 * Replaces the vendor modules: tuxedo_keyboard, uniwill_wmi,
 * uniwill_keyboard, and uniwill_leds — eliminating the inter-module
 * dependency that prevented keyboard backlight from coexisting with
 * our fan control.
 *
 * EC access paths:
 *   Primary:  ACPI methods ECRR/ECRW on \_SB.INOU
 *   Fallback: WMI evaluate on ABBC0F6F GUID (Uniwill BC)
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/module.h>
#include <linux/platform_device.h>
#include <linux/acpi.h>
#include <linux/wmi.h>
#include <linux/mutex.h>
#include <linux/delay.h>
#include <linux/input.h>
#include <linux/input/sparse-keymap.h>
#include <linux/leds.h>
#include <linux/led-class-multicolor.h>
#include <linux/dmi.h>
#include <linux/i8042.h>
#include <linux/serio.h>
#include <linux/workqueue.h>
#include <acpi/battery.h>

#define DRIVER_NAME		"tuxedo-uniwill"

/* ================================================================== */
/* Uniwill WMI GUIDs                                                  */
/* ================================================================== */

#define UW_WMI_GUID_BA		"ABBC0F6D-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_GUID_BB		"ABBC0F6E-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_GUID_BC		"ABBC0F6F-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_EVT_0		"ABBC0F70-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_EVT_1		"ABBC0F71-8EA1-11D1-00A0-C90629100000"
#define UW_WMI_EVT_2		"ABBC0F72-8EA1-11D1-00A0-C90629100000"

/* WMI method constants */
#define UW_WMI_INSTANCE		0x00
#define UW_WMI_METHOD_ID		0x04
#define UW_WMI_FN_WRITE	0
#define UW_WMI_FN_READ	1
#define UW_WMI_FN_FEATURE_TOGGLE 5
#define UW_WMI_ERROR		0xfefefefe
#define UW_WMI_LOCAL_DIMMING_ON 0x0E
#define UW_WMI_LOCAL_DIMMING_OFF 0x0D

/* ================================================================== */
/* EC register addresses                                              */
/* ================================================================== */

/* Fan control */
#define EC_FAN0_PWM		0x1804
#define EC_FAN1_PWM		0x1809
#define EC_CPU_TEMP		0x043e
#define EC_GPU_TEMP		0x044f
#define EC_MODE		0x0751
#define EC_MODE_MANUAL_BIT	6

/* Universal EC fan table */
#define EC_FEATS		0x078e
#define EC_FEATS_UNIV_FAN_BIT	6
#define EC_FEATS_CHG_PROF_BIT	3

#define EC_CHG_PRIO_FEATS		0x0742
#define EC_CHG_PRIO_FEATS_BIT	5

#define EC_CHG_PROFILE		0x07a6
#define EC_CHG_PRIORITY		0x07cc

#define EC_CUSTOM_FAN_CFG0		0x07c5
#define EC_CUSTOM_FAN_CFG0_BIT	7
#define EC_CUSTOM_FAN_CFG1		0x07c6
#define EC_CUSTOM_FAN_CFG1_BIT	2

#define EC_CPU_FAN_END_TEMP		0x0f00
#define EC_CPU_FAN_START_TEMP		0x0f10
#define EC_CPU_FAN_SPEED		0x0f20
#define EC_GPU_FAN_END_TEMP		0x0f30
#define EC_GPU_FAN_START_TEMP		0x0f40
#define EC_GPU_FAN_SPEED		0x0f50

/* Keyboard backlight */
#define EC_KBD_BL_STATUS		0x078c
#define EC_KBD_BL_STATUS_SUBCMD_RESET		0x10
#define EC_KBD_BL_STATUS_BIT_WHITE_ONLY		0x01
#define EC_KBD_BL_RGB_BLUE_IMM		0x1808
#define EC_KBD_BL_RGB_MODE		0x0767
#define EC_KBD_BL_RGB_APPLY		0x20
#define EC_KBD_BL_RGB_RED		0x0769
#define EC_KBD_BL_RGB_GREEN		0x076a
#define EC_KBD_BL_RGB_BLUE		0x076b

/* Keyboard features */
#define EC_BAREBONE_ID		0x0740
#define EC_FEATURES_1		0x0766
#define EC_FEATURES_1_1ZONE_RGB		BIT(2)
#define EC_FEATURES_1_FC5_EN		BIT(5)

/* Fn lock */
#define EC_FN_LOCK		0x074e
#define EC_FN_LOCK_MASK		0x10

/* Charging */
#define EC_AC_AUTO_BOOT		0x0726
#define EC_USB_POWERSHARE		0x0767

/* Battery */
#define EC_BATTERY_CYCN_HI		0x04A7
#define EC_BATTERY_CYCN_LO		0x04A6
#define EC_BATTERY_XIF1_HI		0x0403
#define EC_BATTERY_XIF1_LO		0x0402
#define EC_BATTERY_XIF2_HI		0x0405
#define EC_BATTERY_XIF2_LO		0x0404

/* Mini-LED */
#define EC_MINI_LED_SUPPORT		0x0D4F

/* Lightbar */
#define EC_LIGHTBAR_R		0x0749
#define EC_LIGHTBAR_G		0x074a
#define EC_LIGHTBAR_B		0x074b
#define EC_LIGHTBAR_ANIM		0x0748

/* Custom profile */
#define EC_CUSTOM_PROFILE		0x0727

/* Barebone ID values (for keyboard backlight type detection) */
#define BBID_PFxxxxx		0x09
#define BBID_PFxMxxx		0x0e
#define BBID_PH4TRX1		0x12
#define BBID_PH4TUX1		0x13
#define BBID_PH4TQx1		0x14
#define BBID_PH6TRX1		0x15
#define BBID_PH6TQxx		0x16
#define BBID_PH4Axxx		0x17
#define BBID_PH4Pxxx		0x18

/* WMI OSD / key event codes */
#define UW_OSD_RADIOON		0x01A
#define UW_OSD_RADIOOFF		0x01B
#define UW_OSD_KB_LED_LEVEL0		0x03B
#define UW_OSD_KB_LED_LEVEL1		0x03C
#define UW_OSD_KB_LED_LEVEL2		0x03D
#define UW_OSD_KB_LED_LEVEL3		0x03E
#define UW_OSD_KB_LED_LEVEL4		0x03F
#define UW_OSD_DC_ADAPTER		0x0AB
#define UW_OSD_MODE_CHANGE		0x0B0
#define UW_KEY_RFKILL		0x0A4
#define UW_KEY_KBDILLUMDOWN		0x0B1
#define UW_KEY_KBDILLUMUP		0x0B2
#define UW_KEY_FN_LOCK		0x0B8
#define UW_KEY_KBDILLUMTOGGLE		0x0B9
#define UW_OSD_TOUCHPAD_WA		0xFFF

/* ================================================================== */
/* Fan speed bounds                                                   */
/* ================================================================== */

#define UW_FAN_DUTY_MIN	20
#define UW_FAN_DUTY_MAX	200

/* ================================================================== */
/* Keyboard backlight types                                           */
/* ================================================================== */

enum uw_kb_backlight_type {
	KB_BL_NONE,
	KB_BL_WHITE,/* Fixed color, 3 levels (0-2) */
	KB_BL_WHITE_5,/* Fixed color, 5 levels (0-4) */
	KB_BL_1ZONE_RGB,/* 1-zone RGB, 5 brightness levels */
};

/* ================================================================== */
/* Module state                                                       */
/* ================================================================== */

static DEFINE_MUTEX(ec_lock);
static struct platform_device *pdev;
static acpi_handle inou_handle;
static bool use_inou;
static bool has_univ_fan;
static bool has_chg_profile;
static bool has_chg_priority;
static bool fans_initialized;

/* Keyboard backlight */
static enum uw_kb_backlight_type kb_bl_type = KB_BL_NONE;
static u8 barebone_id;
static bool kb_bl_ec_controlled;
static bool kb_leds_initialized;
static u8 kbd_bl_enable_state_on_start = 0xff;

/* Lightbar */
static bool lightbar_loaded;

/* Input device */
static struct input_dev *uw_input_dev;

/* Mini-LED */
static bool mini_led_last_value;

/* Battery hook */
static bool battery_hook_registered;

/* WMI event */
static bool wmi_evt_installed;

/* Feature flags */
static bool has_ac_auto_boot;
static bool has_usb_powershare;
static bool has_lightbar;
static bool has_mini_led;
static bool has_custom_profile_mode;
static bool has_fn_lock;

/* ================================================================== */
/* EC access: ACPI INOU path (primary)                                */
/* ================================================================== */

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

/* ================================================================== */
/* EC access: WMI path (fallback)                                     */
/* ================================================================== */

static int wmi_evaluate_call(u8 function, u32 arg, u32 *result)
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

	ret = wmi_evaluate_call(UW_WMI_FN_READ, (u32)addr, &result);
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

	ret = wmi_evaluate_call(UW_WMI_FN_WRITE,
   ((u32)data << 16) | (u32)addr, &result);
	if (ret)
		return ret;
	if (result == UW_WMI_ERROR)
		return -EIO;

	return 0;
}

/* ================================================================== */
/* Unified EC access — raw (must hold ec_lock)                        */
/* ================================================================== */

static int __ec_read(u16 addr, u8 *data)
{
	int ret;

	ret = use_inou ? __ec_read_inou(addr, data)
		       : __ec_read_wmi(addr, data);
	usleep_range(5000, 7000);
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

static int uw_ec_write(u16 addr, u8 data)
{
	int ret;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_write(addr, data);
	mutex_unlock(&ec_lock);
	return ret;
}

static int uw_ec_read_u16(u16 hi_addr, u16 lo_addr, u16 *data)
{
	u8 hi, lo;
	int ret;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_read(hi_addr, &hi);
	if (!ret)
		ret = __ec_read(lo_addr, &lo);
	mutex_unlock(&ec_lock);

	if (!ret)
		*data = ((u16)hi << 8) | lo;
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

/* ================================================================== */
/* Universal EC fan table setup                                       */
/* ================================================================== */

static int __uw_init_fan_table(void)
{
	u8 val;
	int ret, i;
	int temp_offset;

	if (fans_initialized)
		return 0;

	ret = __ec_read(EC_MODE, &val);
	if (ret)
		return ret;
	if (val & (1 << EC_MODE_MANUAL_BIT)) {
		val &= ~(1 << EC_MODE_MANUAL_BIT);
		ret = __ec_write(EC_MODE, val);
		if (ret)
			return ret;
	}

	ret = __ec_read(EC_CUSTOM_FAN_CFG0, &val);
	if (ret)
		return ret;
	if (!(val & (1 << EC_CUSTOM_FAN_CFG0_BIT))) {
		val |= (1 << EC_CUSTOM_FAN_CFG0_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG0, val);
		if (ret)
			return ret;
	}

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

static int __uw_restore_auto(void)
{
	u8 val;
	int ret;

	ret = __ec_read(EC_CUSTOM_FAN_CFG1, &val);
	if (!ret && (val & (1 << EC_CUSTOM_FAN_CFG1_BIT))) {
		val &= ~(1 << EC_CUSTOM_FAN_CFG1_BIT);
		__ec_write(EC_CUSTOM_FAN_CFG1, val);
	}

	ret = __ec_read(EC_MODE, &val);
	if (!ret && (val & (1 << EC_MODE_MANUAL_BIT))) {
		val &= ~(1 << EC_MODE_MANUAL_BIT);
		__ec_write(EC_MODE, val);
	}

	fans_initialized = false;
	return 0;
}

/* ================================================================== */
/* Keyboard backlight LED helpers                                     */
/* ================================================================== */

static int uw_write_kbd_bl_brightness(u8 brightness)
{
	u8 data;
	int ret;

	ret = uw_ec_read(EC_KBD_BL_STATUS, &data);
	if (ret)
		return ret;
	data &= 0x0f;
	data |= brightness << 5;
	data |= EC_KBD_BL_STATUS_SUBCMD_RESET;
	return uw_ec_write(EC_KBD_BL_STATUS, data);
}

static int uw_write_kbd_bl_brightness_white_wa(u8 brightness)
{
	u8 data;
	int ret;

	/*
	 * Pulse Gen1/2 workaround: when backlight is off, writing
	 * to 0x078c doesn't apply until next keypress or a poke to
 * the immediate brightness register.
 */
ret = uw_ec_read(EC_KBD_BL_RGB_BLUE_IMM, &data);
if (!ret && !data && brightness)
uw_ec_write(EC_KBD_BL_RGB_BLUE_IMM, 0x01);

return uw_write_kbd_bl_brightness(brightness);
}

/* Convert 0-255 user range to 0-50 EC range */
static u8 uw_rgb_convert(u8 input)
{
return input * 200 / (255 * 4);
}

static int uw_write_kbd_bl_color(u8 red, u8 green, u8 blue)
{
u8 data;
int ret;

ret = uw_ec_write(EC_KBD_BL_RGB_RED, uw_rgb_convert(red));
if (ret)
return ret;
ret = uw_ec_write(EC_KBD_BL_RGB_GREEN, uw_rgb_convert(green));
if (ret)
return ret;
ret = uw_ec_write(EC_KBD_BL_RGB_BLUE, uw_rgb_convert(blue));
if (ret)
return ret;

ret = uw_ec_read(EC_KBD_BL_RGB_MODE, &data);
if (ret)
return ret;
return uw_ec_write(EC_KBD_BL_RGB_MODE, data | EC_KBD_BL_RGB_APPLY);
}

static void uw_write_kbd_bl_enable(u8 enable)
{
u8 data;

enable &= 0x01;
if (uw_ec_read(EC_KBD_BL_STATUS, &data))
return;
data &= ~(1 << 1);
data |= (!enable << 1);
uw_ec_write(EC_KBD_BL_STATUS, data);
}

/* ================================================================== */
/* LED class device: white keyboard backlight                         */
/* ================================================================== */

static void uw_led_white_set(struct led_classdev *cdev,
     enum led_brightness brightness)
{
if (uw_write_kbd_bl_brightness_white_wa(brightness))
pr_debug("white brightness set failed\n");
else
cdev->brightness = brightness;
}

static struct led_classdev uw_white_led = {
.name = "white:" LED_FUNCTION_KBD_BACKLIGHT,
.max_brightness = 2,
.brightness_set = uw_led_white_set,
.brightness = 0,
};

/* ================================================================== */
/* LED class device: 1-zone RGB keyboard backlight                    */
/* ================================================================== */

static void uw_led_mc_set(struct led_classdev *cdev,
  enum led_brightness brightness)
{
struct led_classdev_mc *mcled = lcdev_to_mccdev(cdev);

if (mcled->subled_info[0].intensity == 0 &&
    mcled->subled_info[1].intensity == 0 &&
    mcled->subled_info[2].intensity == 0) {
if (uw_write_kbd_bl_brightness(0))
pr_debug("mc brightness off failed\n");
} else {
if (uw_write_kbd_bl_color(mcled->subled_info[0].intensity,
 mcled->subled_info[1].intensity,
 mcled->subled_info[2].intensity))
pr_debug("mc color set failed\n");
if (uw_write_kbd_bl_brightness(brightness))
pr_debug("mc brightness set failed\n");
}
cdev->brightness = brightness;
}

static struct mc_subled uw_mc_subleds[3] = {
{ .color_index = LED_COLOR_ID_RED,   .intensity = 0xff, .channel = 0 },
{ .color_index = LED_COLOR_ID_GREEN, .intensity = 0xff, .channel = 0 },
{ .color_index = LED_COLOR_ID_BLUE,  .intensity = 0xff, .channel = 0 },
};

static struct led_classdev_mc uw_mc_led = {
.led_cdev = {
.name = "rgb:" LED_FUNCTION_KBD_BACKLIGHT,
.max_brightness = 4,
.brightness_set = uw_led_mc_set,
.brightness = 0,
},
.num_colors = 3,
.subled_info = uw_mc_subleds,
};

/* ================================================================== */
/* LED class device: lightbar                                         */
/* ================================================================== */

#define LB_MAX_BRIGHTNESS		0x24

static int uw_lightbar_set(struct led_classdev *cdev,
   enum led_brightness brightness)
{
const char *name = cdev->name;

if (strstr(name, "lightbar_rgb:1"))
uw_ec_write(EC_LIGHTBAR_R, brightness);
else if (strstr(name, "lightbar_rgb:2"))
uw_ec_write(EC_LIGHTBAR_G, brightness);
else if (strstr(name, "lightbar_rgb:3"))
uw_ec_write(EC_LIGHTBAR_B, brightness);
else if (strstr(name, "lightbar_animation")) {
u8 val;
if (!uw_ec_read(EC_LIGHTBAR_ANIM, &val)) {
if (brightness)
val |= 0x80;
else
val &= ~0x80;
uw_ec_write(EC_LIGHTBAR_ANIM, val);
}
}
return 0;
}

static enum led_brightness uw_lightbar_get(struct led_classdev *cdev)
{
const char *name = cdev->name;
u8 val = 0;

if (strstr(name, "lightbar_rgb:1"))
uw_ec_read(EC_LIGHTBAR_R, &val);
else if (strstr(name, "lightbar_rgb:2"))
uw_ec_read(EC_LIGHTBAR_G, &val);
else if (strstr(name, "lightbar_rgb:3"))
uw_ec_read(EC_LIGHTBAR_B, &val);
else if (strstr(name, "lightbar_animation")) {
uw_ec_read(EC_LIGHTBAR_ANIM, &val);
return (val & 0x80) ? 1 : 0;
}
return val;
}

static struct led_classdev lightbar_leds[] = {
{
.name = "lightbar_rgb:1:status",
.max_brightness = LB_MAX_BRIGHTNESS,
.brightness_set_blocking = uw_lightbar_set,
.brightness_get = uw_lightbar_get,
},
{
.name = "lightbar_rgb:2:status",
.max_brightness = LB_MAX_BRIGHTNESS,
.brightness_set_blocking = uw_lightbar_set,
.brightness_get = uw_lightbar_get,
},
{
.name = "lightbar_rgb:3:status",
.max_brightness = LB_MAX_BRIGHTNESS,
.brightness_set_blocking = uw_lightbar_set,
.brightness_get = uw_lightbar_get,
},
{
.name = "lightbar_animation::status",
.max_brightness = 1,
.brightness_set_blocking = uw_lightbar_set,
.brightness_get = uw_lightbar_get,
},
};

/* ================================================================== */
/* Keyboard brightness change notification (HW-triggered)             */
/* ================================================================== */

static bool uw_notify_brightness_change(void)
{
u8 data, brightness;

if (!kb_leds_initialized || !kb_bl_ec_controlled)
return false;

uw_ec_read(EC_KBD_BL_STATUS, &data);
brightness = (data >> 5) & 0x07;

if (kb_bl_type == KB_BL_WHITE || kb_bl_type == KB_BL_WHITE_5) {
uw_white_led.brightness = brightness;
led_classdev_notify_brightness_hw_changed(&uw_white_led,
  brightness);
return true;
} else if (kb_bl_type == KB_BL_1ZONE_RGB) {
if (uw_mc_led.led_cdev.brightness == brightness) {
/* Polaris Gen2 workaround: EC doesn't react to
			 * FN+space in manual mode; cycle brightness. */
			if (!uw_write_kbd_bl_brightness((brightness + 1) % 5))
				brightness = (brightness + 1) % 5;
		}
		uw_mc_led.led_cdev.brightness = brightness;
		led_classdev_notify_brightness_hw_changed(&uw_mc_led.led_cdev,
							  brightness);
		return true;
	}
	return false;
}

/* ================================================================== */
/* LED state restore (after suspend/adapter change)                   */
/* ================================================================== */

static void uw_leds_restore_state(void)
{
	if (kb_bl_type == KB_BL_WHITE || kb_bl_type == KB_BL_WHITE_5)
		uw_write_kbd_bl_brightness_white_wa(uw_white_led.brightness);
	else if (kb_bl_type == KB_BL_1ZONE_RGB) {
		uw_write_kbd_bl_color(uw_mc_subleds[0].intensity,
      uw_mc_subleds[1].intensity,
      uw_mc_subleds[2].intensity);
		uw_write_kbd_bl_brightness(uw_mc_led.led_cdev.brightness);
	}
}

/* ================================================================== */
/* Input device keymap                                                */
/* ================================================================== */

static const struct key_entry uw_keymap[] = {
	{ KE_KEY, UW_KEY_RFKILL,  { KEY_RFKILL } },
	{ KE_KEY, UW_OSD_TOUCHPAD_WA,  { KEY_F21 } },
	{ KE_KEY, UW_KEY_KBDILLUMDOWN,  { KEY_KBDILLUMDOWN } },
	{ KE_KEY, UW_KEY_KBDILLUMUP,  { KEY_KBDILLUMUP } },
	{ KE_KEY, UW_KEY_KBDILLUMTOGGLE, { KEY_KBDILLUMTOGGLE } },
	{ KE_KEY, UW_OSD_KB_LED_LEVEL0,  { KEY_KBDILLUMTOGGLE } },
	{ KE_KEY, UW_OSD_KB_LED_LEVEL1,  { KEY_KBDILLUMTOGGLE } },
	{ KE_KEY, UW_OSD_KB_LED_LEVEL2,  { KEY_KBDILLUMTOGGLE } },
	{ KE_KEY, UW_OSD_KB_LED_LEVEL3,  { KEY_KBDILLUMTOGGLE } },
	{ KE_KEY, UW_OSD_KB_LED_LEVEL4,  { KEY_KBDILLUMTOGGLE } },
	{ KE_KEY, UW_KEY_FN_LOCK,  { KEY_FN_ESC } },
	/* Ev bits for mode-change combo */
	{ KE_KEY, 0xffff,  { KEY_F6 } },
	{ KE_KEY, 0xffff,  { KEY_LEFTALT } },
	{ KE_KEY, 0xffff,  { KEY_LEFTMETA } },
	{ KE_END, 0 },
};

/* ================================================================== */
/* WMI event handler                                                  */
/* ================================================================== */

static void uw_touchpad_work_fn(struct work_struct *work);
static DECLARE_WORK(uw_touchpad_work, uw_touchpad_work_fn);

static void uw_touchpad_work_fn(struct work_struct *work)
{
	msleep(50);
	if (uw_input_dev)
		sparse_keymap_report_event(uw_input_dev,
						 UW_OSD_TOUCHPAD_WA,
						 1, true);
}

static void uw_set_custom_profile_mode(bool zero_first)
{
	u8 data;

	if (!has_custom_profile_mode)
		return;

	uw_ec_read(EC_CUSTOM_PROFILE, &data);
	if (zero_first) {
		data &= ~(1 << 6);
		uw_ec_write(EC_CUSTOM_PROFILE, data);
		msleep(50);
	}
	data |= (1 << 6);
	uw_ec_write(EC_CUSTOM_PROFILE, data);
}

static void uw_wmi_notify(union acpi_object *data, void *context)
{
	u32 code;

	if (!data || data->type != ACPI_TYPE_INTEGER) {
		pr_debug("wmi event: unexpected data type\n");
		return;
	}
	code = data->integer.value;

	switch (code) {
	case UW_OSD_MODE_CHANGE:
		if (uw_input_dev) {
			input_report_key(uw_input_dev, KEY_LEFTMETA, 1);
			input_report_key(uw_input_dev, KEY_LEFTALT, 1);
			input_report_key(uw_input_dev, KEY_F6, 1);
			input_sync(uw_input_dev);
			input_report_key(uw_input_dev, KEY_F6, 0);
			input_report_key(uw_input_dev, KEY_LEFTALT, 0);
			input_report_key(uw_input_dev, KEY_LEFTMETA, 0);
			input_sync(uw_input_dev);
		}
		break;
	case UW_OSD_DC_ADAPTER:
		uw_set_custom_profile_mode(false);
		uw_leds_restore_state();
		break;
	case UW_KEY_KBDILLUMTOGGLE:
	case UW_OSD_KB_LED_LEVEL0:
	case UW_OSD_KB_LED_LEVEL1:
	case UW_OSD_KB_LED_LEVEL2:
	case UW_OSD_KB_LED_LEVEL3:
	case UW_OSD_KB_LED_LEVEL4:
		if (uw_notify_brightness_change())
			return;
		fallthrough;
	default:
		if (uw_input_dev)
			if (!sparse_keymap_report_event(uw_input_dev,
      code, 1, true))
				pr_debug("unknown event code %#x\n", code);
		break;
	}
}

/* ================================================================== */
/* Touchpad toggle i8042 filter                                       */
/* ================================================================== */

static const u8 touchpad_seq[] = {
	0xe0, 0x5b, 0x1d, 0x76, 0xf6, 0x9d, 0xe0, 0xdb
};

static bool uw_i8042_filter(unsigned char data, unsigned char str,
    struct serio *port, void *context)
{
	static u8 seq_pos;

	if (unlikely(str & I8042_STR_AUXDATA))
		return false;

	if (unlikely(data == touchpad_seq[seq_pos])) {
		++seq_pos;
		if (unlikely(data == 0x76 || data == 0xf6))
			return true;
		else if (unlikely(seq_pos == ARRAY_SIZE(touchpad_seq))) {
			schedule_work(&uw_touchpad_work);
			seq_pos = 0;
		}
		return false;
	}

	seq_pos = 0;
	return false;
}

/* ================================================================== */
/* Fan sysfs attributes                                               */
/* ================================================================== */

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
	if (has_univ_fan)
		return sysfs_emit(buf, "%u\n", fans_initialized ? 1 : 0);
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

/* ================================================================== */
/* Fan table sysfs                                                    */
/* ================================================================== */

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

	while (n_zones < 16 && *p) {
		unsigned int t, s;
		int chars;

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

	ret = __ec_read(EC_CUSTOM_FAN_CFG1, &val);
	if (ret)
		goto out;
	if (val & (1 << EC_CUSTOM_FAN_CFG1_BIT)) {
		val &= ~(1 << EC_CUSTOM_FAN_CFG1_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG1, val);
		if (ret)
			goto out;
	}

	ret = __ec_read(EC_MODE, &val);
	if (ret)
		goto out;
	if (val & (1 << EC_MODE_MANUAL_BIT)) {
		val &= ~(1 << EC_MODE_MANUAL_BIT);
		ret = __ec_write(EC_MODE, val);
		if (ret)
			goto out;
	}

	ret = __ec_read(EC_CUSTOM_FAN_CFG0, &val);
	if (ret)
		goto out;
	if (!(val & (1 << EC_CUSTOM_FAN_CFG0_BIT))) {
		val |= (1 << EC_CUSTOM_FAN_CFG0_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG0, val);
		if (ret)
			goto out;
	}

	for (i = 0; i < n_zones; i++) {
		u8 start = (i == 0) ? 0 : end_temps[i - 1];

		ret = __ec_write(EC_CPU_FAN_END_TEMP + i, end_temps[i]);
		if (ret) goto out;
		ret = __ec_write(EC_CPU_FAN_START_TEMP + i, start);
		if (ret) goto out;
		ret = __ec_write(EC_CPU_FAN_SPEED + i, speeds[i]);
		if (ret) goto out;

		ret = __ec_write(EC_GPU_FAN_END_TEMP + i, end_temps[i]);
		if (ret) goto out;
		ret = __ec_write(EC_GPU_FAN_START_TEMP + i, start);
		if (ret) goto out;
		ret = __ec_write(EC_GPU_FAN_SPEED + i, speeds[i]);
		if (ret) goto out;
	}

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

	ret = __ec_read(EC_CUSTOM_FAN_CFG1, &val);
	if (!ret) {
		val |= (1 << EC_CUSTOM_FAN_CFG1_BIT);
		ret = __ec_write(EC_CUSTOM_FAN_CFG1, val);
	}

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

/* ================================================================== */
/* Charging profile/priority sysfs attributes                         */
/* ================================================================== */

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

	for (i = 0; i < ARRAY_SIZE(chg_profile_names); i++) {
		if (sysfs_streq(buf, chg_profile_names[i]))
			break;
	}
	if (i >= ARRAY_SIZE(chg_profile_names))
		return -EINVAL;

	val = (u8)i;

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

/* ================================================================== */
/* Fn lock sysfs                                                      */
/* ================================================================== */

static ssize_t fn_lock_show(struct device *dev,
    struct device_attribute *attr, char *buf)
{
	u8 data;
	int ret;

	ret = uw_ec_read(EC_FN_LOCK, &data);
	if (ret)
		return ret;

	return sysfs_emit(buf, "%d\n", (data & EC_FN_LOCK_MASK) ? 1 : 0);
}

static ssize_t fn_lock_store(struct device *dev,
     struct device_attribute *attr,
     const char *buf, size_t count)
{
	int on, ret;
	u8 data;

	if (kstrtoint(buf, 10, &on) || on < 0 || on > 1)
		return -EINVAL;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_read(EC_FN_LOCK, &data);
	if (!ret) {
		if (on)
			data |= EC_FN_LOCK_MASK;
		else
			data &= ~EC_FN_LOCK_MASK;
		ret = __ec_write(EC_FN_LOCK, data);
	}
	mutex_unlock(&ec_lock);
	return ret ? ret : count;
}

static DEVICE_ATTR_RW(fn_lock);

/* ================================================================== */
/* AC auto boot sysfs                                                 */
/* ================================================================== */

static ssize_t ac_auto_boot_show(struct device *dev,
 struct device_attribute *attr, char *buf)
{
	u8 val;
	int ret;

	ret = uw_ec_read(EC_AC_AUTO_BOOT, &val);
	if (ret)
		return ret;

	return sysfs_emit(buf, "%d\n", (val >> 3) & 0x01);
}

static ssize_t ac_auto_boot_store(struct device *dev,
  struct device_attribute *attr,
  const char *buf, size_t count)
{
	u8 val, reg;
	int ret;

	if (kstrtou8(buf, 10, &val) || val > 1)
		return -EINVAL;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_read(EC_AC_AUTO_BOOT, &reg);
	if (!ret) {
		reg = (reg & ~(1 << 3)) | ((val & 0x01) << 3);
		ret = __ec_write(EC_AC_AUTO_BOOT, reg);
	}
	mutex_unlock(&ec_lock);
	return ret ? ret : count;
}

static DEVICE_ATTR_RW(ac_auto_boot);

/* ================================================================== */
/* USB power share sysfs                                              */
/* ================================================================== */

static ssize_t usb_powershare_show(struct device *dev,
   struct device_attribute *attr, char *buf)
{
	u8 val;
	int ret;

	ret = uw_ec_read(EC_USB_POWERSHARE, &val);
	if (ret)
		return ret;

	return sysfs_emit(buf, "%d\n", (val >> 4) & 0x01);
}

static ssize_t usb_powershare_store(struct device *dev,
    struct device_attribute *attr,
    const char *buf, size_t count)
{
	u8 val, reg;
	int ret;

	if (kstrtou8(buf, 10, &val) || val > 1)
		return -EINVAL;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	ret = __ec_read(EC_USB_POWERSHARE, &reg);
	if (!ret) {
		reg = (reg & ~(1 << 4)) | ((val & 0x01) << 4);
		ret = __ec_write(EC_USB_POWERSHARE, reg);
	}
	mutex_unlock(&ec_lock);
	return ret ? ret : count;
}

static DEVICE_ATTR_RW(usb_powershare);

/* ================================================================== */
/* Mini-LED local dimming sysfs                                       */
/* ================================================================== */

static ssize_t mini_led_dimming_show(struct device *dev,
     struct device_attribute *attr, char *buf)
{
	return sysfs_emit(buf, "%d\n", mini_led_last_value ? 1 : 0);
}

static ssize_t mini_led_dimming_store(struct device *dev,
      struct device_attribute *attr,
      const char *buf, size_t count)
{
	u8 val;
	u32 result;
	int ret;

	if (kstrtou8(buf, 10, &val) || val > 1)
		return -EINVAL;

	ret = wmi_evaluate_call(UW_WMI_FN_FEATURE_TOGGLE,
   val ? UW_WMI_LOCAL_DIMMING_ON
       : UW_WMI_LOCAL_DIMMING_OFF,
   &result);
	if (ret)
		return ret;

	mini_led_last_value = val;
	return count;
}

static DEVICE_ATTR_RW(mini_led_dimming);

/* ================================================================== */
/* Battery ACPI hook                                                  */
/* ================================================================== */

static ssize_t raw_cycle_count_show(struct device *dev,
    struct device_attribute *attr, char *buf)
{
	u16 cycle;
	int ret;

	ret = uw_ec_read_u16(EC_BATTERY_CYCN_HI, EC_BATTERY_CYCN_LO, &cycle);
	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", cycle);
}

static ssize_t raw_xif1_show(struct device *dev,
     struct device_attribute *attr, char *buf)
{
	u16 xif;
	int ret;

	ret = uw_ec_read_u16(EC_BATTERY_XIF1_HI, EC_BATTERY_XIF1_LO, &xif);
	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", xif);
}

static ssize_t raw_xif2_show(struct device *dev,
     struct device_attribute *attr, char *buf)
{
	u16 xif;
	int ret;

	ret = uw_ec_read_u16(EC_BATTERY_XIF2_HI, EC_BATTERY_XIF2_LO, &xif);
	if (ret)
		return ret;
	return sysfs_emit(buf, "%u\n", xif);
}

static DEVICE_ATTR_RO(raw_cycle_count);
static DEVICE_ATTR_RO(raw_xif1);
static DEVICE_ATTR_RO(raw_xif2);

static struct attribute *uw_battery_attrs[] = {
	&dev_attr_raw_cycle_count.attr,
	&dev_attr_raw_xif1.attr,
	&dev_attr_raw_xif2.attr,
	NULL,
};

ATTRIBUTE_GROUPS(uw_battery);

static int uw_battery_add(struct power_supply *battery,
  struct acpi_battery_hook *hook)
{
	if (device_add_groups(&battery->dev, uw_battery_groups))
		return -ENODEV;
	return 0;
}

static int uw_battery_remove(struct power_supply *battery,
     struct acpi_battery_hook *hook)
{
	device_remove_groups(&battery->dev, uw_battery_groups);
	return 0;
}

static struct acpi_battery_hook uw_battery_hook = {
	.add_battery = uw_battery_add,
	.remove_battery = uw_battery_remove,
	.name = "TUXEDO Battery Extension",
};

/* ================================================================== */
/* Platform sysfs attribute group                                     */
/* ================================================================== */

static struct attribute *uw_attrs[] = {
	&dev_attr_fan0_pwm.attr,
	&dev_attr_fan1_pwm.attr,
	&dev_attr_fan_mode.attr,
	&dev_attr_cpu_temp.attr,
	&dev_attr_gpu_temp.attr,
	&dev_attr_fan_count.attr,
	&dev_attr_fan_table.attr,
	&dev_attr_charge_profile.attr,
	&dev_attr_charge_priority.attr,
	&dev_attr_fn_lock.attr,
	&dev_attr_ac_auto_boot.attr,
	&dev_attr_usb_powershare.attr,
	&dev_attr_mini_led_dimming.attr,
	NULL,
};

static umode_t uw_attr_visible(struct kobject *kobj,
       struct attribute *attr, int n)
{
	if (attr == &dev_attr_charge_profile.attr && !has_chg_profile)
		return 0;
	if (attr == &dev_attr_charge_priority.attr && !has_chg_priority)
		return 0;
	if (attr == &dev_attr_fn_lock.attr && !has_fn_lock)
		return 0;
	if (attr == &dev_attr_ac_auto_boot.attr && !has_ac_auto_boot)
		return 0;
	if (attr == &dev_attr_usb_powershare.attr && !has_usb_powershare)
		return 0;
	if (attr == &dev_attr_mini_led_dimming.attr && !has_mini_led)
		return 0;
	return attr->mode;
}

static const struct attribute_group uw_attr_group = {
	.attrs = uw_attrs,
	.is_visible = uw_attr_visible,
};

/* ================================================================== */
/* DMI support tables                                                 */
/* ================================================================== */

static const struct dmi_system_id force_no_ec_led_control[] = {
	{
		.matches = {
			DMI_MATCH(DMI_SYS_VENDOR, "TUXEDO"),
			DMI_MATCH(DMI_PRODUCT_SKU, "STELLARIS1XA05"),
		},
	},
	{
		.matches = {
			DMI_MATCH(DMI_SYS_VENDOR, "TUXEDO"),
			DMI_MATCH(DMI_PRODUCT_SKU, "STELLSL15I06"),
		},
	},
	{ }
};

static const struct dmi_system_id dmi_5level_white[] = {
	{
		.matches = {
			DMI_MATCH(DMI_SYS_VENDOR, "TUXEDO"),
			DMI_MATCH(DMI_BOARD_NAME, "GXxHRXx"),
		},
	},
	{
		.matches = {
			DMI_MATCH(DMI_SYS_VENDOR, "TUXEDO"),
			DMI_MATCH(DMI_BOARD_NAME, "GXxMRXx"),
		},
	},
	{
		.matches = {
			DMI_MATCH(DMI_SYS_VENDOR, "TUXEDO"),
			DMI_MATCH(DMI_BOARD_NAME, "XxHP4NAx"),
		},
	},
	{
		.matches = {
			DMI_MATCH(DMI_SYS_VENDOR, "TUXEDO"),
			DMI_MATCH(DMI_BOARD_NAME, "XxKK4NAx_XxSP4NAx"),
		},
	},
	{
		.matches = {
			DMI_MATCH(DMI_SYS_VENDOR, "TUXEDO"),
			DMI_MATCH(DMI_BOARD_NAME, "XxAR4NAx"),
		},
	},
	{ }
};

static bool dmi_lightbar_supported(void)
{
	return dmi_match(DMI_BOARD_NAME, "LAPQC71A")
	    || dmi_match(DMI_BOARD_NAME, "LAPQC71B")
	    || dmi_match(DMI_BOARD_NAME, "TRINITY1501I")
	    || dmi_match(DMI_BOARD_NAME, "TRINITY1701I")
	    || dmi_match(DMI_PRODUCT_NAME, "A60 MUV")
	    || dmi_match(DMI_PRODUCT_SKU, "STELLARIS1XI03")
	    || dmi_match(DMI_PRODUCT_SKU, "STELLARIS1XA03")
	    || dmi_match(DMI_PRODUCT_SKU, "STELLARIS1XI04")
	    || dmi_match(DMI_PRODUCT_SKU, "STEPOL1XA04");
}

static bool dmi_auto_boot_powershare_supported(void)
{
	return dmi_match(DMI_BOARD_NAME, "GXxMRXx")
	    || dmi_match(DMI_BOARD_NAME, "GXxHRXx")
	    || dmi_match(DMI_BOARD_NAME, "XxHP4NAx")
	    || dmi_match(DMI_BOARD_NAME, "XxKK4NAx_XxSP4NAx")
	    || dmi_match(DMI_BOARD_NAME, "XxAR4NAx")
	    || dmi_match(DMI_BOARD_NAME, "GM6IXxB_MB1")
	    || dmi_match(DMI_BOARD_NAME, "GM6IXxB_MB2")
	    || dmi_match(DMI_BOARD_NAME, "GM7IXxN")
	    || dmi_match(DMI_BOARD_NAME, "X6AR5xxY")
	    || dmi_match(DMI_BOARD_NAME, "X6AR5xxY_mLED")
	    || dmi_match(DMI_BOARD_NAME, "X6FR5xxY")
	    || dmi_match(DMI_BOARD_NAME, "GMxHGxx")
	    || dmi_match(DMI_BOARD_NAME, "GM5IXxA")
	    || dmi_match(DMI_BOARD_NAME, "X5KK45xS_X5SP45xS");
}

static bool dmi_fn_lock_excluded(void)
{
	return dmi_match(DMI_BOARD_NAME, "LAPQC71A")
	    || dmi_match(DMI_BOARD_NAME, "LAPQC71B")
	    || dmi_match(DMI_PRODUCT_NAME, "A60 MUV");
}

static bool dmi_custom_profile_needed(void)
{
	return dmi_match(DMI_PRODUCT_SKU, "STELLARIS16I06")
	    || dmi_match(DMI_PRODUCT_SKU, "STELLARIS17I06")
	    || dmi_match(DMI_PRODUCT_SKU, "STELLARIS16I07")
	    || dmi_match(DMI_PRODUCT_SKU, "STELLARIS16A07")
	    || dmi_match(DMI_PRODUCT_SKU, "STELLSL15I06")
	    || dmi_match(DMI_PRODUCT_SKU, "STELLSL15A06")
	    || dmi_match(DMI_BOARD_NAME, "GXxMRXx")
	    || dmi_match(DMI_BOARD_NAME, "GXxHRXx")
	    || dmi_match(DMI_BOARD_NAME, "XxHP4NAx")
	    || dmi_match(DMI_BOARD_NAME, "XxKK4NAx_XxSP4NAx")
	    || dmi_match(DMI_BOARD_NAME, "X5KK45xS_X5SP45xS")
	    || dmi_match(DMI_BOARD_NAME, "X6AR55xU")
	    || dmi_match(DMI_BOARD_NAME, "X5AR45xS");
}

/* ================================================================== */
/* Feature detection                                                  */
/* ================================================================== */

static void uw_detect_features(void)
{
	u8 feats, data;

	/* Fan control features */
	has_univ_fan = false;
	has_chg_profile = false;
	has_chg_priority = false;
	if (!uw_ec_read(EC_FEATS, &feats)) {
		has_univ_fan = (feats >> EC_FEATS_UNIV_FAN_BIT) & 1;
		has_chg_profile = (feats >> EC_FEATS_CHG_PROF_BIT) & 1;
	}
	if (!uw_ec_read(EC_CHG_PRIO_FEATS, &feats))
		has_chg_priority = (feats >> EC_CHG_PRIO_FEATS_BIT) & 1;

	/* Fn lock — test read to confirm support */
	has_fn_lock = false;
	if (!dmi_fn_lock_excluded()) {
		u8 fn_data;
		if (!uw_ec_read(EC_FN_LOCK, &fn_data))
			has_fn_lock = true;
	}

	/* AC auto boot + USB power share */
	has_ac_auto_boot = dmi_auto_boot_powershare_supported();
	has_usb_powershare = dmi_auto_boot_powershare_supported();

	/* Lightbar */
	has_lightbar = dmi_lightbar_supported();

	/* Mini-LED local dimming */
	has_mini_led = false;
	if (!uw_ec_read(EC_MINI_LED_SUPPORT, &data))
		has_mini_led = (data != 0xFF) && (data & 0x01);

	/* Custom profile mode */
	has_custom_profile_mode = dmi_custom_profile_needed();

	/* Keyboard backlight type */
	kb_bl_type = KB_BL_NONE;
	kb_bl_ec_controlled = false;

	if (!uw_ec_read(EC_BAREBONE_ID, &barebone_id) && barebone_id) {
		/* Enable fixed-color-5 support bit (needed on IBP Gen7-9) */
		if (!uw_ec_read(EC_FEATURES_1, &data)) {
			data |= EC_FEATURES_1_FC5_EN;
			uw_ec_write(EC_FEATURES_1, data);
		}

		if (dmi_check_system(force_no_ec_led_control)) {
			/* Skip LED registration on quirked devices */
		} else if (barebone_id == BBID_PFxxxxx ||
			   barebone_id == BBID_PFxMxxx ||
			   barebone_id == BBID_PH4TRX1 ||
			   barebone_id == BBID_PH4TUX1 ||
			   barebone_id == BBID_PH4TQx1 ||
			   barebone_id == BBID_PH6TRX1 ||
			   barebone_id == BBID_PH6TQxx ||
			   barebone_id == BBID_PH4Axxx ||
			   barebone_id == BBID_PH4Pxxx) {
			kb_bl_type = KB_BL_WHITE;
			kb_bl_ec_controlled = true;
		} else if (dmi_check_system(dmi_5level_white)) {
			kb_bl_type = KB_BL_WHITE_5;
			kb_bl_ec_controlled = true;
		} else {
			if (!uw_ec_read(EC_FEATURES_1, &data) &&
			    (data & EC_FEATURES_1_1ZONE_RGB)) {
				kb_bl_type = KB_BL_1ZONE_RGB;
				kb_bl_ec_controlled = true;
			}
		}
	}

	pr_info("features: fan=%s chg_prof=%s chg_prio=%s fn_lock=%s "
"ac_boot=%s usb_share=%s lightbar=%s mini_led=%s "
"kb_bl=%d bbid=%#04x\n",
has_univ_fan ? "universal" : "legacy",
has_chg_profile ? "yes" : "no",
has_chg_priority ? "yes" : "no",
has_fn_lock ? "yes" : "no",
has_ac_auto_boot ? "yes" : "no",
has_usb_powershare ? "yes" : "no",
has_lightbar ? "yes" : "no",
has_mini_led ? "yes" : "no",
kb_bl_type, barebone_id);
}

/* ================================================================== */
/* LED init/remove                                                    */
/* ================================================================== */

static int uw_leds_init(void)
{
	int ret, i, j;

	/* Keyboard backlight */
	if (kb_bl_type == KB_BL_WHITE) {
		uw_white_led.max_brightness = 2;
		if (kb_bl_ec_controlled)
			uw_white_led.flags = LED_BRIGHT_HW_CHANGED;
		ret = led_classdev_register(&pdev->dev, &uw_white_led);
		if (ret) {
			pr_err("failed to register white kbd LED\n");
			return ret;
		}
	} else if (kb_bl_type == KB_BL_WHITE_5) {
		uw_white_led.max_brightness = 4;
		if (kb_bl_ec_controlled)
			uw_white_led.flags = LED_BRIGHT_HW_CHANGED;
		ret = led_classdev_register(&pdev->dev, &uw_white_led);
		if (ret) {
			pr_err("failed to register white-5 kbd LED\n");
			return ret;
		}
	} else if (kb_bl_type == KB_BL_1ZONE_RGB) {
		if (kb_bl_ec_controlled)
			uw_mc_led.led_cdev.flags = LED_BRIGHT_HW_CHANGED;
		ret = led_classdev_multicolor_register(&pdev->dev,
						       &uw_mc_led);
		if (ret) {
			pr_err("failed to register RGB kbd LED\n");
			return ret;
		}
	}

	kb_leds_initialized = (kb_bl_type != KB_BL_NONE);

	/* Lightbar */
	if (has_lightbar) {
		for (i = 0; i < ARRAY_SIZE(lightbar_leds); i++) {
			ret = led_classdev_register(&pdev->dev,
						    &lightbar_leds[i]);
			if (ret) {
				for (j = 0; j < i; j++)
					led_classdev_unregister(
&lightbar_leds[j]);
				pr_err("failed to register lightbar LED %d\n",
       i);
				return ret;
			}
		}
		lightbar_loaded = true;
		/* Default: animation off, LEDs off */
		uw_ec_write(EC_LIGHTBAR_ANIM, 0);
		uw_ec_write(EC_LIGHTBAR_R, 0);
		uw_ec_write(EC_LIGHTBAR_G, 0);
		uw_ec_write(EC_LIGHTBAR_B, 0);
	}

	return 0;
}

static void uw_leds_remove(void)
{
	int i;

	if (kb_leds_initialized) {
		kb_leds_initialized = false;
		if (kb_bl_type == KB_BL_WHITE ||
		    kb_bl_type == KB_BL_WHITE_5)
			led_classdev_unregister(&uw_white_led);
		else if (kb_bl_type == KB_BL_1ZONE_RGB)
			led_classdev_multicolor_unregister(&uw_mc_led);
	}

	if (lightbar_loaded) {
		for (i = 0; i < ARRAY_SIZE(lightbar_leds); i++)
			led_classdev_unregister(&lightbar_leds[i]);
		lightbar_loaded = false;
	}
}

/* ================================================================== */
/* Input device init/remove                                           */
/* ================================================================== */

static int uw_input_init(void)
{
	int ret;

	uw_input_dev = input_allocate_device();
	if (!uw_input_dev)
		return -ENOMEM;

	uw_input_dev->name = "TUXEDO Keyboard";
	uw_input_dev->phys = DRIVER_NAME "/input0";
	uw_input_dev->id.bustype = BUS_HOST;
	uw_input_dev->dev.parent = &pdev->dev;

	ret = sparse_keymap_setup(uw_input_dev, uw_keymap, NULL);
	if (ret) {
		input_free_device(uw_input_dev);
		uw_input_dev = NULL;
		return ret;
	}

	ret = input_register_device(uw_input_dev);
	if (ret) {
		input_free_device(uw_input_dev);
		uw_input_dev = NULL;
		return ret;
	}

	return 0;
}

static void uw_input_remove(void)
{
	if (uw_input_dev) {
		input_unregister_device(uw_input_dev);
		uw_input_dev = NULL;
	}
}

/* ================================================================== */
/* Module init / exit                                                 */
/* ================================================================== */

static int __init uw_init(void)
{
	acpi_status status;
	int ret;
	u8 data;

	/* Detect EC access path */
	if (wmi_has_guid(UW_WMI_GUID_BA) &&
	    wmi_has_guid(UW_WMI_GUID_BB) &&
	    wmi_has_guid(UW_WMI_GUID_BC) &&
	    wmi_has_guid(UW_WMI_EVT_0)  &&
	    wmi_has_guid(UW_WMI_EVT_1)  &&
	    wmi_has_guid(UW_WMI_EVT_2)) {
		use_inou = false;
	} else {
		status = acpi_get_handle(NULL, "\\_SB.INOU", &inou_handle);
		if (ACPI_FAILURE(status)) {
			pr_debug("no Uniwill interface found\n");
			return -ENODEV;
		}
		use_inou = true;
	}

	/* Detect all features */
	uw_detect_features();

	/* EC initialization */
	mutex_lock(&ec_lock);
	__ec_write(0x0751, 0x00);/* balanced profile */
	__ec_write(0x0741, 0x01);/* enable manual/custom mode */
	__ec_write(EC_GPU_TEMP, 0x00);/* clear for fan detection */
	mutex_unlock(&ec_lock);

	/* Custom profile mode */
	uw_set_custom_profile_mode(true);

	/* Register platform device */
	pdev = platform_device_register_simple(DRIVER_NAME, -1, NULL, 0);
	if (IS_ERR(pdev))
		return PTR_ERR(pdev);

	ret = sysfs_create_group(&pdev->dev.kobj, &uw_attr_group);
	if (ret)
		goto err_pdev;

	/* Save keyboard backlight enable state, then enable */
	if (kb_bl_type != KB_BL_NONE) {
		uw_ec_read(EC_KBD_BL_STATUS, &data);
		kbd_bl_enable_state_on_start = (data >> 1) & 0x01;
	}

	/* Register LED class devices */
	ret = uw_leds_init();
	if (ret)
		goto err_sysfs;

	/* Enable keyboard backlight */
	if (kb_bl_type != KB_BL_NONE)
		uw_write_kbd_bl_enable(1);

	/* Register input device (non-fatal — Fn keys are supplementary) */
	ret = uw_input_init();
	if (ret)
		pr_warn("input device registration failed: %d\n", ret);

	/* Install WMI event handler */
	if (wmi_has_guid(UW_WMI_EVT_2)) {
		status = wmi_install_notify_handler(UW_WMI_EVT_2,
    uw_wmi_notify, NULL);
		if (ACPI_SUCCESS(status))
			wmi_evt_installed = true;
		else
			pr_warn("WMI event handler install failed\n");
	}

	/* Install i8042 touchpad toggle filter */
	if (i8042_install_filter(uw_i8042_filter, NULL))
		pr_debug("i8042 filter already active\n");

	/* Mini-LED: default off */
	if (has_mini_led) {
		u32 result;
		wmi_evaluate_call(UW_WMI_FN_FEATURE_TOGGLE,
     UW_WMI_LOCAL_DIMMING_OFF, &result);
		mini_led_last_value = false;
	}

	/* Battery hook */
	battery_hook_register(&uw_battery_hook);
	battery_hook_registered = true;

	pr_info("initialized (%s interface, %s fan control)\n",
use_inou ? "INOU" : "WMI",
has_univ_fan ? "universal" : "legacy");
	return 0;

err_sysfs:
	sysfs_remove_group(&pdev->dev.kobj, &uw_attr_group);
err_pdev:
	platform_device_unregister(pdev);
	return ret;
}

static void __exit uw_exit(void)
{
	/* Restore EC auto control */
	if (has_univ_fan && fans_initialized) {
		mutex_lock(&ec_lock);
		__uw_restore_auto();
		mutex_unlock(&ec_lock);
	}

	/* Remove i8042 filter */
	i8042_remove_filter(uw_i8042_filter);
	cancel_work_sync(&uw_touchpad_work);

	/* Remove WMI event handler */
	if (wmi_evt_installed)
		wmi_remove_notify_handler(UW_WMI_EVT_2);

	/* Unregister battery hook */
	if (battery_hook_registered)
		battery_hook_unregister(&uw_battery_hook);

	/* Remove input device */
	uw_input_remove();

	/* Remove LEDs */
	uw_leds_remove();

	/* Restore keyboard backlight enable state */
	if (kbd_bl_enable_state_on_start != 0xff)
		uw_write_kbd_bl_enable(kbd_bl_enable_state_on_start);

	/* Disable manual mode (only if we enabled it) */
	if (has_univ_fan)
		uw_ec_write(0x0741, 0x00);

	sysfs_remove_group(&pdev->dev.kobj, &uw_attr_group);
	platform_device_unregister(pdev);
	pr_info("removed\n");
}

module_init(uw_init);
module_exit(uw_exit);

MODULE_AUTHOR("TUXEDO Computers GmbH <tux@tuxedocomputers.com>");
MODULE_DESCRIPTION("Unified Uniwill platform driver (fan, keyboard, LED, charging, battery)");
MODULE_LICENSE("GPL");
MODULE_ALIAS("wmi:" UW_WMI_GUID_BC);
