#!/bin/bash
# ==============================================================================
# install-sd.sh - Xiaomi Redmi 9A (blossom/dandelion) Alpine Linux SD Installer
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

ALPINE_VERSION="3.20.3"
ALPINE_MIRROR="https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/aarch64"
ALPINE_TAR="alpine-minirootfs-${ALPINE_VERSION}-aarch64.tar.gz"
DOWNLOAD_URL="${ALPINE_MIRROR}/${ALPINE_TAR}"
CACHE_DIR="/tmp/alpine_blossom_cache"

if [ "$(id -u)" -ne 0 ]; then
	echo "Error: This script must be run as root (sudo ./install-sd.sh <target-disk>)" >&2
	exit 1
fi

TARGET_DEV="$1"

if [ -z "$TARGET_DEV" ]; then
	echo "Usage: sudo $0 <target-block-device>"
	echo "Example: sudo $0 /dev/sdb   (or /dev/mmcblk1)"
	echo ""
	echo "Available removable disks:"
	lsblk -d -o NAME,SIZE,TYPE,TRAN,RM,MODEL | grep -E 'usb|mmc|1$' || true
	exit 1
fi

if [ ! -b "$TARGET_DEV" ]; then
	echo "Error: '$TARGET_DEV' is not a valid block device." >&2
	exit 1
fi

# Safety guard: Prevent accidental overwrite of system disk
ROOT_DEV="$(findmnt -n -o SOURCE / | sed -E 's/p?[0-9]+$//')"
if [ "$TARGET_DEV" = "$ROOT_DEV" ]; then
	echo "CRITICAL ERROR: '$TARGET_DEV' is your host OS root drive! Aborting." >&2
	exit 1
fi

echo "======================================================================"
echo "TARGET DEVICE: $TARGET_DEV"
lsblk "$TARGET_DEV"
echo "======================================================================"
echo "WARNING: ALL DATA ON $TARGET_DEV WILL BE PERMANENTLY ERASED!"
read -r -p "Are you sure you want to proceed? Type 'YES' to continue: " CONFIRM
if [ "$CONFIRM" != "YES" ]; then
	echo "Aborted by user."
	exit 1
fi

# 1. Unmount existing partitions on target
echo "[1/6] Unmounting any mounted partitions on $TARGET_DEV..."
umount "${TARGET_DEV}"* 2>/dev/null || true

# 2. Partition disk
echo "[2/6] Partitioning $TARGET_DEV..."
wipefs -a "$TARGET_DEV"
parted -s "$TARGET_DEV" mklabel msdos
parted -s -a optimal "$TARGET_DEV" mkpart primary ext4 2048s 100%

# Determine partition node name (e.g. /dev/sdb1 vs /dev/mmcblk1p1)
if [[ "$TARGET_DEV" =~ [0-9]$ ]]; then
	PART_DEV="${TARGET_DEV}p1"
else
	PART_DEV="${TARGET_DEV}1"
fi

# Wait for kernel to update partition table
sleep 2

# 3. Format ext4 with label linux-rootfs
echo "[3/6] Formatting $PART_DEV as ext4 (LABEL=linux-rootfs)..."
mkfs.ext4 -F -L "linux-rootfs" "$PART_DEV"

# 4. Download Alpine minirootfs if needed
mkdir -p "$CACHE_DIR"
if [ ! -f "$CACHE_DIR/$ALPINE_TAR" ]; then
	echo "[4/6] Downloading Alpine Linux ${ALPINE_VERSION} aarch64 minirootfs..."
	wget -c "$DOWNLOAD_URL" -O "$CACHE_DIR/$ALPINE_TAR"
else
	echo "[4/6] Using cached Alpine minirootfs: $CACHE_DIR/$ALPINE_TAR"
fi

# 5. Mount and extract
MOUNT_DIR="/mnt/blossom_alpine_rootfs_$$"
mkdir -p "$MOUNT_DIR"
echo "[5/6] Extracting rootfs to $MOUNT_DIR..."
mount "$PART_DEV" "$MOUNT_DIR"

tar -xpf "$CACHE_DIR/$ALPINE_TAR" --numeric-owner -C "$MOUNT_DIR"

# 6. Apply overlay files
echo "[6/6] Applying device overlay..."
if [ -d "$REPO_DIR/overlay" ]; then
	cp -av "$REPO_DIR/overlay/"* "$MOUNT_DIR/"
fi

# Set proper permissions
chmod 755 "$MOUNT_DIR/usr/bin/sxmo_deviceprofile_mediatek,MT6765.sh" 2>/dev/null || true
chmod 755 "$MOUNT_DIR/usr/bin/sxmo_jackmon" 2>/dev/null || true
chmod 755 "$MOUNT_DIR/usr/bin/sxmo_audio.sh" 2>/dev/null || true
chmod 755 "$MOUNT_DIR/usr/local/bin/mtk-fb-blank" 2>/dev/null || true
chmod +x "$MOUNT_DIR/etc/local.d/"*.start 2>/dev/null || true
chmod +x "$MOUNT_DIR/home/user/.config/sxmo/hooks/"*.sh 2>/dev/null || true
chmod 755 "$MOUNT_DIR/home/user/.xinitrc" 2>/dev/null || true

# Set nameserver for first boot internet
echo "nameserver 1.1.1.1" > "$MOUNT_DIR/etc/resolv.conf"

sync
umount "$MOUNT_DIR"
rmdir "$MOUNT_DIR"

echo "======================================================================"
echo "Installation complete!"
echo "Your MicroSD card is now prepared for Xiaomi Redmi 9A."
echo ""
echo "Next steps:"
echo "1. Insert the MicroSD card into your Redmi 9A."
echo "2. Boot phone into fastboot mode (Hold Volume Down + Power)."
echo "3. Flash the native Linux kernel:"
echo "   fastboot flash boot boot-linux.img"
echo "4. Reboot:"
echo "   fastboot reboot"
echo "======================================================================"
