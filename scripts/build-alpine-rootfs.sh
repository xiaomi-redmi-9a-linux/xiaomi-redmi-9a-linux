#!/bin/bash
# ==============================================================================
# build-alpine-rootfs.sh - Build 5 GiB ext4 rootfs image for Xiaomi Redmi 9A
# Compatible with loopback boot (/dev/loop0) on microSD external storage
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configuration & Defaults
OUT_DIR="${OUT_DIR:-${REPO_DIR}/out}"
IMAGE="${1:-${OUT_IMAGE:-${OUT_DIR}/linux-rootfs.ext4}}"
IMAGE_SIZE="${IMAGE_SIZE:-5G}"
IMAGE_LABEL="${IMAGE_LABEL:-alpine-rootfs}"
ALPINE_VERSION="${ALPINE_VERSION:-3.20.3}"
ALPINE_MIRROR="${ALPINE_MIRROR:-https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/aarch64}"
ALPINE_TAR="alpine-minirootfs-${ALPINE_VERSION}-aarch64.tar.gz"
DOWNLOAD_URL="${ALPINE_MIRROR}/${ALPINE_TAR}"
CACHE_DIR="${CACHE_DIR:-/tmp/alpine_blossom_cache}"

# Detect base root: prefer existing pre-configured root if available, otherwise temp dir
LOCAL_TEST_ROOT="${REPO_DIR}/../alpine-rootfs/test_root"
if [ -n "${BASE_ROOT:-}" ] && [ -d "$BASE_ROOT" ]; then
    SRC_ROOT="$BASE_ROOT"
elif [ -d "$LOCAL_TEST_ROOT" ] && [ "${FRESH_BOOTSTRAP:-0}" != "1" ]; then
    SRC_ROOT="$LOCAL_TEST_ROOT"
else
    SRC_ROOT=""
fi

BUILD_DIR="/tmp/blossom_rootfs_build_$$"
cleanup() {
    if [ -d "$BUILD_DIR" ]; then
        rm -rf "$BUILD_DIR"
    fi
}
trap cleanup EXIT

echo "======================================================================"
echo " Xiaomi Redmi 9A (blossom / dandelion) Rootfs Image Builder"
echo " Target Output : $IMAGE"
echo " Target Size   : $IMAGE_SIZE"
echo " Source Base   : ${SRC_ROOT:-Fresh Alpine ${ALPINE_VERSION} Minirootfs}"
echo "======================================================================"

# Verify host dependencies
for tool in fakeroot mkfs.ext4 fallocate; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Error: Required host tool '$tool' is not installed." >&2
        exit 1
    fi
done

mkdir -p "$OUT_DIR"
mkdir -p "$BUILD_DIR"

# 1. Populate base root
if [ -n "$SRC_ROOT" ]; then
    echo "[1/8] Syncing base rootfs from $SRC_ROOT..."
    cp -a "$SRC_ROOT"/* "$BUILD_DIR/"
else
    mkdir -p "$CACHE_DIR"
    if [ ! -f "$CACHE_DIR/$ALPINE_TAR" ]; then
        echo "[1/8] Downloading Alpine Linux ${ALPINE_VERSION} aarch64 minirootfs..."
        wget -c "$DOWNLOAD_URL" -O "$CACHE_DIR/$ALPINE_TAR"
    else
        echo "[1/8] Using cached Alpine minirootfs: $CACHE_DIR/$ALPINE_TAR"
    fi
    echo "       Extracting minirootfs into build tree..."
    tar -xpf "$CACHE_DIR/$ALPINE_TAR" --numeric-owner -C "$BUILD_DIR"
fi

# 2. Configure Users & Inittab Autologin
echo "[2/8] Configuring users and inittab autologin..."
# Allow root login without password for headless debug
if [ -f "$BUILD_DIR/etc/shadow" ]; then
    sed -i 's|^root:[^:]*:|root::|' "$BUILD_DIR/etc/shadow"
fi

# Configure inittab for emergency console autologin
cat << 'INITTAB' > "$BUILD_DIR/etc/inittab"
::sysinit:/sbin/openrc sysinit
::wait:/sbin/openrc boot
::wait:/sbin/openrc default

tty1::respawn:/bin/sh -l
ttyGS0::respawn:/bin/sh -l
tty0::respawn:/bin/sh -l

::ctrlaltdel:/sbin/reboot
::shutdown:/sbin/openrc shutdown
INITTAB

# Ensure non-root 'user' exists with UID 1000
if ! grep -q "^user:" "$BUILD_DIR/etc/passwd" 2>/dev/null; then
    echo "user:x:1000:1000:User:/home/user:/bin/sh" >> "$BUILD_DIR/etc/passwd"
    echo "user::19000:0:99999:7:::" >> "$BUILD_DIR/etc/shadow"
    echo "user:x:1000:" >> "$BUILD_DIR/etc/group"
fi
for grp in wheel audio video input dialout plugdev disk; do
    if grep -q "^${grp}:" "$BUILD_DIR/etc/group" 2>/dev/null; then
        sed -i "s/^${grp}:.*/&,user/" "$BUILD_DIR/etc/group"
        sed -i "s/,user,user/,user/" "$BUILD_DIR/etc/group"
    else
        echo "${grp}:x:999:user" >> "$BUILD_DIR/etc/group"
    fi
done

# 3. Setup OpenRC Runlevels
echo "[3/8] Setting up OpenRC service links..."
mkdir -p "$BUILD_DIR/etc/runlevels/sysinit" "$BUILD_DIR/etc/runlevels/boot" "$BUILD_DIR/etc/runlevels/default"

for s in devfs dmesg udev udev-trigger udev-settle; do
    [ -f "$BUILD_DIR/etc/init.d/$s" ] && ln -sf "/etc/init.d/$s" "$BUILD_DIR/etc/runlevels/sysinit/$s"
done

for s in bootmisc hostname modules sysctl syslog wpa_supplicant wpa_cli networking urandom mtk-wmt-loader; do
    [ -f "$BUILD_DIR/etc/init.d/$s" ] && ln -sf "/etc/init.d/$s" "$BUILD_DIR/etc/runlevels/boot/$s"
done

for s in local dbus bluetooth mtk-bt-bridge mtk-sensor-proxy brightnessctl sshd tinydm; do
    [ -f "$BUILD_DIR/etc/init.d/$s" ] && ln -sf "/etc/init.d/$s" "$BUILD_DIR/etc/runlevels/default/$s"
done

# 4. Hostname, DNS, and FSTAB
echo "[4/8] Configuring hostname, DNS, and fstab..."
echo "alpine-blossom" > "$BUILD_DIR/etc/hostname"

cat << 'RESOLV' > "$BUILD_DIR/etc/resolv.conf"
nameserver 1.1.1.1
nameserver 8.8.8.8
RESOLV

cat << 'FSTAB' > "$BUILD_DIR/etc/fstab"
/dev/loop0      /       ext4    defaults,noatime,errors=remount-ro 0 1
/dev/mmcblk1p1  /boot   auto    defaults,noatime 0 0
FSTAB

# 5. Network & Sanitized Wi-Fi
echo "[5/8] Configuring networking and Wi-Fi..."
cat << 'INTERFACES' > "$BUILD_DIR/etc/network/interfaces"
auto lo
iface lo inet loopback

auto wlan0
iface wlan0 inet dhcp
INTERFACES

mkdir -p "$BUILD_DIR/etc/conf.d"
cat << 'WPACONF_D' > "$BUILD_DIR/etc/conf.d/wpa_supplicant"
wpa_supplicant_if="wlan0"
wpa_supplicant_args="-Dnl80211"
wpa_supplicant_conf="/etc/wpa_supplicant/wpa_supplicant.conf"
wpa_supplicant_dbus=no
WPACONF_D

cat << 'WPACLI_D' > "$BUILD_DIR/etc/conf.d/wpa_cli"
WPACLI_OPTS="-a /etc/wpa_supplicant/wpa_cli.sh"
WPACLI_D

mkdir -p "$BUILD_DIR/etc/wpa_supplicant"
if [ -n "${WIFI_SSID:-}" ] && [ -n "${WIFI_PSK:-}" ]; then
    cat << WPAEOF > "$BUILD_DIR/etc/wpa_supplicant/wpa_supplicant.conf"
ctrl_interface=/var/run/wpa_supplicant
update_config=1

network={
	ssid="${WIFI_SSID}"
	psk="${WIFI_PSK}"
}
WPAEOF
else
    cat << 'WPAEOF' > "$BUILD_DIR/etc/wpa_supplicant/wpa_supplicant.conf"
ctrl_interface=/var/run/wpa_supplicant
update_config=1

# Configure Wi-Fi via:
#   wpa_passphrase "SSID" "PASSWORD" >> /etc/wpa_supplicant/wpa_supplicant.conf
#   rc-service wpa_supplicant restart
WPAEOF
fi

cat << 'WPACLISH' > "$BUILD_DIR/etc/wpa_supplicant/wpa_cli.sh"
#!/bin/sh
# Event hook for wpa_supplicant -> udhcpc on Alpine Linux
IFNAME="${1}"
ACTION="${2}"
DHCPIDFILE="/var/run/udhcpc.${IFNAME}.pid"

case "${ACTION}" in
	CONNECTED)
		logger -t wpa_cli "Interface ${IFNAME} CONNECTED: acquiring DHCP lease..."
		if [ -e "${DHCPIDFILE}" ]; then
			read -r DHCPID < "${DHCPIDFILE}"
			kill -USR1 "${DHCPID}" 2>/dev/null || udhcpc -b -R -p "${DHCPIDFILE}" -i "${IFNAME}"
		else
			udhcpc -b -R -p "${DHCPIDFILE}" -i "${IFNAME}"
		fi
		;;
	DISCONNECTED)
		logger -t wpa_cli "Interface ${IFNAME} DISCONNECTED"
		if [ -e "${DHCPIDFILE}" ]; then
			read -r DHCPID < "${DHCPIDFILE}"
			kill -USR2 "${DHCPID}" 2>/dev/null
		fi
		;;
esac
WPACLISH
chmod +x "$BUILD_DIR/etc/wpa_supplicant/wpa_cli.sh"

# 6. SSH Configuration & Optional Authorized Key
echo "[6/8] Configuring SSH server..."
if [ -f "$BUILD_DIR/etc/ssh/sshd_config" ]; then
    sed -i 's/^#PermitRootLogin.*/PermitRootLogin yes/' "$BUILD_DIR/etc/ssh/sshd_config"
    sed -i 's/^#PubkeyAuthentication.*/PubkeyAuthentication yes/' "$BUILD_DIR/etc/ssh/sshd_config"
    sed -i 's/^#PasswordAuthentication.*/PasswordAuthentication yes/' "$BUILD_DIR/etc/ssh/sshd_config"
    grep -q "PermitEmptyPasswords yes" "$BUILD_DIR/etc/ssh/sshd_config" || echo "PermitEmptyPasswords yes" >> "$BUILD_DIR/etc/ssh/sshd_config"
fi

mkdir -p "$BUILD_DIR/root/.ssh" "$BUILD_DIR/home/user/.ssh"
chmod 0700 "$BUILD_DIR/root/.ssh" "$BUILD_DIR/home/user/.ssh"

# Install host SSH key if provided or detected
HOST_KEY=""
if [ -n "${SSH_PUBKEY_FILE:-}" ] && [ -f "$SSH_PUBKEY_FILE" ]; then
    HOST_KEY="$SSH_PUBKEY_FILE"
elif [ -f "$HOME/.ssh/id_ed25519.pub" ]; then
    HOST_KEY="$HOME/.ssh/id_ed25519.pub"
elif [ -f "$HOME/.ssh/id_rsa.pub" ]; then
    HOST_KEY="$HOME/.ssh/id_rsa.pub"
fi
if [ -n "$HOST_KEY" ]; then
    echo "       Pre-authorizing SSH public key from $HOST_KEY"
    cat "$HOST_KEY" > "$BUILD_DIR/root/.ssh/authorized_keys"
    cat "$HOST_KEY" > "$BUILD_DIR/home/user/.ssh/authorized_keys"
    chmod 0600 "$BUILD_DIR/root/.ssh/authorized_keys" "$BUILD_DIR/home/user/.ssh/authorized_keys"
fi

# 7. Apply Companion Repository Overlay & Firmwares
echo "[7/8] Applying repository overlay and firmwares..."
if [ -d "$REPO_DIR/overlay" ]; then
    cp -av "$REPO_DIR/overlay/"* "$BUILD_DIR/"
fi

# Fix permissions
find "$BUILD_DIR/usr/bin" -type f -name "sxmo_*" -exec chmod 755 {} + 2>/dev/null || true
find "$BUILD_DIR/usr/local/bin" -type f -exec chmod 755 {} + 2>/dev/null || true
find "$BUILD_DIR/etc/local.d" -type f -name "*.start" -exec chmod 755 {} + 2>/dev/null || true
find "$BUILD_DIR/home/user/.config/sxmo/hooks" -type f -name "*.sh" -exec chmod 755 {} + 2>/dev/null || true
[ -f "$BUILD_DIR/home/user/.xinitrc" ] && chmod 755 "$BUILD_DIR/home/user/.xinitrc"
# Ensure SUID on mtk_fm daemon for non-root audio control
[ -f "$BUILD_DIR/usr/local/bin/mtk_fm" ] && chmod 4755 "$BUILD_DIR/usr/local/bin/mtk_fm"

# Copy kernel firmwares if available
FW_DIR="${FW_DIR:-${REPO_DIR}/../kernel_xiaomi_blossom_linux/firmware}"
if [ -d "$FW_DIR" ]; then
    echo "       Embedding device firmwares from $FW_DIR..."
    mkdir -p "$BUILD_DIR/lib/firmware"
    cp -an "$FW_DIR"/* "$BUILD_DIR/lib/firmware/" 2>/dev/null || true
    mkdir -p "$BUILD_DIR/vendor" "$BUILD_DIR/system/etc"
    ln -sfT /lib/firmware "$BUILD_DIR/vendor/firmware" 2>/dev/null || true
    ln -sfT /lib/firmware "$BUILD_DIR/system/etc/firmware" 2>/dev/null || true
fi

# Gralloc Rogue EGL symlinks
mkdir -p "$BUILD_DIR/vendor/lib64/hw" "$BUILD_DIR/system/lib64/hw"
ln -sfT /vendor "$BUILD_DIR/system/vendor" 2>/dev/null || true
ln -sf gralloc.rogue.so "$BUILD_DIR/vendor/lib64/hw/gralloc.default.so" 2>/dev/null || true
ln -sf gralloc.rogue.so "$BUILD_DIR/vendor/lib64/hw/gralloc.mt6765.so" 2>/dev/null || true
ln -sf /vendor/lib64/hw/gralloc.rogue.so "$BUILD_DIR/system/lib64/hw/gralloc.default.so" 2>/dev/null || true

# 8. Create ext4 loop image
echo "[8/8] Generating ext4 filesystem image ($IMAGE_SIZE)..."
rm -f "$IMAGE"
fallocate -l "$IMAGE_SIZE" "$IMAGE"
fakeroot mkfs.ext4 -O ^orphan_file -F -L "$IMAGE_LABEL" -d "$BUILD_DIR" "$IMAGE"
chmod 0666 "$IMAGE"

echo "======================================================================"
echo " SUCCESS: Rootfs image created successfully!"
echo " Location: $IMAGE"
ls -lh "$IMAGE"
echo "======================================================================"
