#!/bin/sh
IFNAME="${1}"
ACTION="${2}"
DHCPIDFILE="/var/run/udhcpc.${IFNAME}.pid"

case "${ACTION}" in
	CONNECTED)
		if grep -q "iface ${IFNAME} inet static" /etc/network/interfaces 2>/dev/null; then
			logger -t wpa_cli "Interface ${IFNAME} CONNECTED: applying static IP 192.168.1.81..."
			ip addr replace 192.168.1.81/24 dev "${IFNAME}"
			ip route replace default via 192.168.1.1 dev "${IFNAME}"
			if [ ! -f /etc/resolv.conf ] || ! grep -q "nameserver" /etc/resolv.conf; then
				echo "nameserver 192.168.1.1" > /etc/resolv.conf
			fi
		else
			logger -t wpa_cli "Interface ${IFNAME} CONNECTED: acquiring DHCP lease..."
			udhcpc -b -R -p "${DHCPIDFILE}" -i "${IFNAME}"
		fi
		;;
	DISCONNECTED)
		logger -t wpa_cli "Interface ${IFNAME} DISCONNECTED"
		;;
esac
