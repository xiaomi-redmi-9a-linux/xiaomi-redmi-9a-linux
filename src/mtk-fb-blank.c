/*
 * mtk-fb-blank.c - Simple utility to blank / unblank MediaTek framebuffer
 *
 * Usage:
 *   mtk-fb-blank on   - Calls ioctl(FBIOBLANK, FB_BLANK_UNBLANK)
 *   mtk-fb-blank off  - Calls ioctl(FBIOBLANK, FB_BLANK_POWERDOWN)
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <linux/fb.h>

int main(int argc, char *argv[])
{
	int fd;
	int blank_mode;

	if (argc < 2) {
		fprintf(stderr, "Usage: %s <on|off>\n", argv[0]);
		return 1;
	}

	if (strcmp(argv[1], "on") == 0) {
		blank_mode = FB_BLANK_UNBLANK;
	} else if (strcmp(argv[1], "off") == 0) {
		blank_mode = FB_BLANK_POWERDOWN;
	} else {
		fprintf(stderr, "Invalid argument '%s'. Use 'on' or 'off'.\n", argv[1]);
		return 1;
	}

	fd = open("/dev/fb0", O_RDWR);
	if (fd < 0) {
		perror("open /dev/fb0");
		return 1;
	}

	if (ioctl(fd, FBIOBLANK, blank_mode) < 0) {
		perror(blank_mode == FB_BLANK_UNBLANK ? "ioctl FB_BLANK_UNBLANK" : "ioctl FB_BLANK_POWERDOWN");
		close(fd);
		return 1;
	}

	close(fd);
	return 0;
}
