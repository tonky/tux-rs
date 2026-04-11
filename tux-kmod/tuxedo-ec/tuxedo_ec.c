// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * tuxedo_ec - NB05 EC raw memory access via SuperIO I2EC
 *
 * Exposes the entire 64 KiB EC RAM as a binary sysfs attribute at
 * /sys/devices/platform/tuxedo-ec/ec_ram.  The daemon uses pread/pwrite
 * at offset = EC address (0x0000–0xFFFF) to access individual registers.
 *
 * No semantic attributes — all interpretation (fan duty, temp, RPM
 * register addresses) lives in the userspace daemon.
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/module.h>
#include <linux/platform_device.h>
#include <linux/mutex.h>
#include <linux/sysfs.h>
#include <asm/io.h>

#define DRIVER_NAME		"tuxedo-ec"

/* SuperIO indirect I/O ports */
#define EC_PORT_ADDR		0x4e
#define EC_PORT_DATA		0x4f

/* I2EC addressing registers (accessed via port 0x2e/0x2f) */
#define I2EC_REG_ADDR		0x2e
#define I2EC_REG_DATA		0x2f
#define I2EC_ADDR_LOW		0x10
#define I2EC_ADDR_HIGH		0x11
#define I2EC_ADDR_DATA		0x12

/* EC RAM size */
#define EC_RAM_SIZE		(64 * 1024)

static DEFINE_MUTEX(ec_lock);
static struct platform_device *pdev;
static bool ports_requested;

/* ------------------------------------------------------------------ */
/* Low-level I/O                                                      */
/* ------------------------------------------------------------------ */

static void io_write(u8 reg, u8 data)
{
	outb(reg, EC_PORT_ADDR);
	outb(data, EC_PORT_DATA);
}

static u8 io_read(u8 reg)
{
	outb(reg, EC_PORT_ADDR);
	return inb(EC_PORT_DATA);
}

/*
 * Read a single byte from EC RAM at a 16-bit address using I2EC protocol.
 * Must be called with ec_lock held.
 */
static u8 __ec_read(u16 addr)
{
	/* Set address high byte */
	io_write(I2EC_REG_ADDR, I2EC_ADDR_HIGH);
	io_write(I2EC_REG_DATA, (addr >> 8) & 0xff);

	/* Set address low byte */
	io_write(I2EC_REG_ADDR, I2EC_ADDR_LOW);
	io_write(I2EC_REG_DATA, addr & 0xff);

	/* Read data */
	io_write(I2EC_REG_ADDR, I2EC_ADDR_DATA);
	return io_read(I2EC_REG_DATA);
}

/*
 * Write a single byte to EC RAM at a 16-bit address using I2EC protocol.
 * Must be called with ec_lock held.
 */
static void __ec_write(u16 addr, u8 data)
{
	/* Set address high byte */
	io_write(I2EC_REG_ADDR, I2EC_ADDR_HIGH);
	io_write(I2EC_REG_DATA, (addr >> 8) & 0xff);

	/* Set address low byte */
	io_write(I2EC_REG_ADDR, I2EC_ADDR_LOW);
	io_write(I2EC_REG_DATA, addr & 0xff);

	/* Write data */
	io_write(I2EC_REG_ADDR, I2EC_ADDR_DATA);
	io_write(I2EC_REG_DATA, data);
}

/* ------------------------------------------------------------------ */
/* Binary sysfs attribute: ec_ram (64 KiB)                            */
/* ------------------------------------------------------------------ */

static ssize_t ec_ram_read(struct file *filp, struct kobject *kobj,
			   const struct bin_attribute *attr,
			   char *buf, loff_t off, size_t count)
{
	size_t i;

	if (off >= EC_RAM_SIZE)
		return 0;
	if (off + count > EC_RAM_SIZE)
		count = EC_RAM_SIZE - off;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	for (i = 0; i < count; i++)
		buf[i] = __ec_read((u16)(off + i));
	mutex_unlock(&ec_lock);

	return count;
}

static ssize_t ec_ram_write(struct file *filp, struct kobject *kobj,
			    const struct bin_attribute *attr,
			    char *buf, loff_t off, size_t count)
{
	size_t i;

	if (off >= EC_RAM_SIZE)
		return -EFBIG;
	if (off + count > EC_RAM_SIZE)
		count = EC_RAM_SIZE - off;

	if (mutex_lock_interruptible(&ec_lock))
		return -ERESTARTSYS;
	for (i = 0; i < count; i++)
		__ec_write((u16)(off + i), buf[i]);
	mutex_unlock(&ec_lock);

	return count;
}

static struct bin_attribute bin_attr_ec_ram =
	__BIN_ATTR(ec_ram, 0600, ec_ram_read, ec_ram_write, EC_RAM_SIZE);

static const struct bin_attribute *const ec_bin_attrs[] = {
	&bin_attr_ec_ram,
	NULL,
};

static const struct attribute_group ec_group = {
	.bin_attrs = ec_bin_attrs,
};

/* ------------------------------------------------------------------ */
/* Module init / exit                                                 */
/* ------------------------------------------------------------------ */

static int __init tuxedo_ec_init(void)
{
	int ret;

	/*
	 * Verify the SuperIO ports are accessible. On non-NB05 hardware
	 * these ports may not exist or belong to a different device.
	 * We do a basic sanity check: read port 0x4e and verify we can
	 * access I2EC registers without error.
	 */
	if (!request_region(EC_PORT_ADDR, 2, DRIVER_NAME)) {
		pr_debug("cannot request I/O ports 0x%x-0x%x\n",
			 EC_PORT_ADDR, EC_PORT_ADDR + 1);
		return -EBUSY;
	}
	ports_requested = true;

	pdev = platform_device_register_simple(DRIVER_NAME, -1, NULL, 0);
	if (IS_ERR(pdev)) {
		ret = PTR_ERR(pdev);
		goto err_release;
	}

	ret = sysfs_create_group(&pdev->dev.kobj, &ec_group);
	if (ret)
		goto err_unregister;

	pr_info("initialized, ec_ram at /sys/devices/platform/%s/ec_ram\n",
		DRIVER_NAME);
	return 0;

err_unregister:
	platform_device_unregister(pdev);
err_release:
	release_region(EC_PORT_ADDR, 2);
	ports_requested = false;
	return ret;
}

static void __exit tuxedo_ec_exit(void)
{
	sysfs_remove_group(&pdev->dev.kobj, &ec_group);
	platform_device_unregister(pdev);
	if (ports_requested)
		release_region(EC_PORT_ADDR, 2);
	pr_info("removed\n");
}

module_init(tuxedo_ec_init);
module_exit(tuxedo_ec_exit);

MODULE_AUTHOR("TUXEDO Computers GmbH <tux@tuxedocomputers.com>");
MODULE_DESCRIPTION("NB05 EC raw memory access via SuperIO I2EC");
MODULE_LICENSE("GPL");
