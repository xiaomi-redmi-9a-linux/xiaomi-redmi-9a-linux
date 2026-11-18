#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <fcntl.h>
#include <unistd.h>
#include <signal.h>
#include <errno.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/stat.h>
#include <sys/select.h>
#include <tinyalsa/asoundlib.h>

#define FM_IOC_MAGIC 0xf5
#define FM_IOCTL_POWERUP   _IOWR(FM_IOC_MAGIC, 0, void*)
#define FM_IOCTL_POWERDOWN _IOWR(FM_IOC_MAGIC, 1, int32_t*)
#define FM_IOCTL_TUNE      _IOWR(FM_IOC_MAGIC, 2, void*)
#define FM_IOCTL_SETVOL    _IOWR(FM_IOC_MAGIC, 4, uint32_t*)
#define FM_IOCTL_MUTE      _IOWR(FM_IOC_MAGIC, 6, uint32_t*)
#define FM_IOCTL_GETRSSI   _IOWR(FM_IOC_MAGIC, 7, int32_t*)

#define SOCK_PATH "/run/mtk_fm.sock"
#define PID_PATH  "/run/mtk_fm.pid"

struct fm_tune_parm {
	uint8_t err;
	uint8_t band;
	uint8_t space;
	uint8_t hilo;
	uint8_t deemphasis;
	uint16_t freq;
};

static int fm_fd = -1;
static struct pcm *pcm_dev = NULL;
static int cur_freq = 9830;
static int cur_vol = 75;
static char cur_route[16] = "headphone";
static bool is_playing = false;
static volatile sig_atomic_t keep_running = 1;

static void notify_statusbar(void) {
	pid_t pid = fork();
	if (pid == 0) {
		setsid();
		execlp("su", "su", "-", "user", "-c", "sxmo_hook_statusbar.sh radio", NULL);
		_exit(0);
	}
}

static void ensure_nodes(void) {
	struct stat st;
	if (stat("/dev/snd/pcmC1D21p", &st) != 0) {
		system("mknod -m 660 /dev/snd/pcmC1D21p c 116 30 2>/dev/null; chown root:audio /dev/snd/pcmC1D21p 2>/dev/null");
	}
}

static void apply_mixer_settings(void) {
	char cmd[256];

	// Hardware digital gain: 0 to 524288 (0x80000)
	int hw_gain = (cur_vol * 524288) / 100;
	if (hw_gain < 0) hw_gain = 0;
	if (hw_gain > 524288) hw_gain = 524288;
	snprintf(cmd, sizeof(cmd), "amixer -c 1 cset numid=101 %d >/dev/null 2>&1", hw_gain);
	system(cmd);

	// Antenna detection must always stay on (headphones act as FM antenna)
	system("amixer -c 1 cset numid=21 1 >/dev/null 2>&1");

	if (strcmp(cur_route, "speaker") == 0) {
		// Route to Built-in Speaker:
		system("amixer -c 1 cset numid=5 0 >/dev/null 2>&1");
		system("amixer -c 1 cset numid=1 0 >/dev/null 2>&1");
		system("amixer -c 1 cset numid=2 0 >/dev/null 2>&1");
		system("amixer -c 1 cset numid=4 0 >/dev/null 2>&1");
		system("amixer -c 1 cset numid=4 1 >/dev/null 2>&1");  // Activates LO buffer & LOL mux
		system("amixer -c 1 cset numid=12 1 >/dev/null 2>&1"); // Pulses external speaker amp
		system("amixer -c 1 set 'Lineout_PGAL_GAIN' -- '4Db' >/dev/null 2>&1");
		system("amixer -c 1 set 'Lineout_PGAR_GAIN' -- '4Db' >/dev/null 2>&1");
		system("amixer -c 1 set 'Headset_PGAL_GAIN' -- '-40Db' >/dev/null 2>&1");
		system("amixer -c 1 set 'Headset_PGAR_GAIN' -- '-40Db' >/dev/null 2>&1");
	} else {
		// Route to Headphones:
		system("amixer -c 1 cset numid=12 0 >/dev/null 2>&1"); // Disable ext speaker amp
		system("amixer -c 1 cset numid=4 0 >/dev/null 2>&1");
		system("amixer -c 1 cset numid=5 0 >/dev/null 2>&1");
		system("amixer -c 1 cset numid=1 0 >/dev/null 2>&1");
		system("amixer -c 1 cset numid=2 0 >/dev/null 2>&1");
		system("amixer -c 1 cset numid=1 1 >/dev/null 2>&1");  // Activates Headphone DAC & amp
		system("amixer -c 1 cset numid=2 1 >/dev/null 2>&1");
		system("amixer -c 1 set 'Headset_PGAL_GAIN' -- '4Db' >/dev/null 2>&1");
		system("amixer -c 1 set 'Headset_PGAR_GAIN' -- '4Db' >/dev/null 2>&1");
		system("amixer -c 1 set 'Lineout_PGAL_GAIN' -- '-40Db' >/dev/null 2>&1");
		system("amixer -c 1 set 'Lineout_PGAR_GAIN' -- '-40Db' >/dev/null 2>&1");
	}
}

static int start_radio(int freq) {
	if (is_playing) return 0;
	ensure_nodes();

	fm_fd = open("/dev/fm", O_RDWR);
	if (fm_fd < 0) {
		perror("open /dev/fm");
		return -1;
	}

	struct fm_tune_parm parm = {0};
	parm.band = 1;  // 87.5 - 108.0 MHz
	parm.space = 1; // 100 kHz
	parm.freq = freq;

	if (ioctl(fm_fd, FM_IOCTL_POWERUP, &parm) < 0) {
		perror("FM_IOCTL_POWERUP");
		close(fm_fd);
		fm_fd = -1;
		return -1;
	}

	uint32_t mute = 0;
	ioctl(fm_fd, FM_IOCTL_MUTE, &mute);
	uint32_t vol = 15;
	ioctl(fm_fd, FM_IOCTL_SETVOL, &vol);

	apply_mixer_settings();

	struct pcm_config config = {
		.channels = 2,
		.rate = 44100,
		.period_size = 1024,
		.period_count = 4,
		.format = PCM_FORMAT_S16_LE
	};

	pcm_dev = pcm_open(1, 21, PCM_OUT, &config);
	if (!pcm_dev || !pcm_is_ready(pcm_dev)) {
		fprintf(stderr, "pcm_open failed: %s\n", pcm_dev ? pcm_get_error(pcm_dev) : "null");
		if (pcm_dev) { pcm_close(pcm_dev); pcm_dev = NULL; }
		int32_t pwr = 0;
		ioctl(fm_fd, FM_IOCTL_POWERDOWN, &pwr);
		close(fm_fd);
		fm_fd = -1;
		return -1;
	}

	if (pcm_prepare(pcm_dev) < 0) {
		fprintf(stderr, "pcm_prepare failed: %s\n", pcm_get_error(pcm_dev));
		pcm_close(pcm_dev);
		pcm_dev = NULL;
		int32_t pwr = 0;
		ioctl(fm_fd, FM_IOCTL_POWERDOWN, &pwr);
		close(fm_fd);
		fm_fd = -1;
		return -1;
	}

	cur_freq = freq;
	is_playing = true;
	notify_statusbar();
	return 0;
}

static void stop_radio(void) {
	if (!is_playing) return;
	if (pcm_dev) {
		pcm_close(pcm_dev);
		pcm_dev = NULL;
	}
	if (fm_fd >= 0) {
		int32_t pwr = 0;
		ioctl(fm_fd, FM_IOCTL_POWERDOWN, &pwr);
		close(fm_fd);
		fm_fd = -1;
	}
	// Mute FM digital gain
	system("amixer -c 1 cset numid=101 0 >/dev/null 2>&1");
	// Restore system audio baseline for all apps
	system("/etc/local.d/01-audio.start >/dev/null 2>&1");
	is_playing = false;
	notify_statusbar();
}

static int tune_radio(int freq) {
	if (!is_playing) {
		return start_radio(freq);
	}
	struct fm_tune_parm parm = {0};
	parm.band = 1;
	parm.space = 1;
	parm.freq = freq;
	if (ioctl(fm_fd, FM_IOCTL_TUNE, &parm) < 0) {
		return -1;
	}
	cur_freq = freq;
	notify_statusbar();
	return 0;
}

static int get_rssi(void) {
	if (!is_playing || fm_fd < 0) return -120;
	int32_t rssi = -120;
	ioctl(fm_fd, FM_IOCTL_GETRSSI, &rssi);
	return rssi;
}

// Fast 200 kHz step-and-sample seek
static int seek_radio(int dir) {
	if (!is_playing || fm_fd < 0) return -1;

	int step = (dir > 0) ? 20 : -20; // 200 kHz step
	int f = cur_freq + step;
	if (f > 10800) f = 8750;
	if (f < 8750)  f = 10800;

	struct fm_tune_parm parm = {0};
	parm.band = 1;
	parm.space = 1;

	for (int i = 0; i < 103; i++) {
		parm.freq = f;
		if (ioctl(fm_fd, FM_IOCTL_TUNE, &parm) == 0) {
			usleep(10000); // 10ms
			int32_t rssi = -120;
			ioctl(fm_fd, FM_IOCTL_GETRSSI, &rssi);
			if (rssi >= -88) { // Station found!
				cur_freq = f;
				notify_statusbar();
				return 0;
			}
		}

		f += step;
		if (f > 10800) f = 8750;
		if (f < 8750)  f = 10800;
	}

	// Restore original station if none found
	parm.freq = cur_freq;
	ioctl(fm_fd, FM_IOCTL_TUNE, &parm);
	return 0;
}

// Clean 200 kHz sweep across 87.5 - 108.0 MHz
static int perform_scan(char *out_buf, size_t max_len) {
	if (!is_playing || fm_fd < 0) {
		snprintf(out_buf, max_len, "ERROR Not playing\n");
		return -1;
	}

	int orig_freq = cur_freq;
	// Mute audio during scan
	system("amixer -c 1 cset numid=101 0 >/dev/null 2>&1");

	struct fm_tune_parm parm = {0};
	parm.band = 1;
	parm.space = 1;

	int count = 0;
	size_t offset = snprintf(out_buf, max_len, "OK SCAN [");

	for (int f = 8750; f <= 10800; f += 20) {
		parm.freq = f;
		if (ioctl(fm_fd, FM_IOCTL_TUNE, &parm) == 0) {
			usleep(8000);
			int32_t rssi = -120;
			ioctl(fm_fd, FM_IOCTL_GETRSSI, &rssi);
			if (rssi >= -88) {
				if (count > 0 && offset < max_len - 32) {
					offset += snprintf(out_buf + offset, max_len - offset, ", ");
				}
				if (offset < max_len - 32) {
					offset += snprintf(out_buf + offset, max_len - offset,
					                   "{\"freq\": %.1f, \"rssi\": %d}", f / 100.0, rssi);
				}
				count++;
			}
		}
	}

	if (offset < max_len - 8) {
		offset += snprintf(out_buf + offset, max_len - offset, "]\n");
	}

	// Restore original station & volume
	parm.freq = orig_freq;
	ioctl(fm_fd, FM_IOCTL_TUNE, &parm);
	cur_freq = orig_freq;
	apply_mixer_settings();

	return 0;
}

static void sig_handler(int sig) {
	(void)sig;
	keep_running = 0;
}

static void handle_client(int client_fd) {
	char buf[256];
	memset(buf, 0, sizeof(buf));
	int n = read(client_fd, buf, sizeof(buf) - 1);
	if (n <= 0) { close(client_fd); return; }

	// Trim whitespace
	while (n > 0 && (buf[n-1] == '\n' || buf[n-1] == '\r' || buf[n-1] == ' ')) buf[--n] = '\0';

	char resp[8192];
	memset(resp, 0, sizeof(resp));

	if (strcasecmp(buf, "STATUS") == 0) {
		if (is_playing) {
			snprintf(resp, sizeof(resp), "PLAYING %.1f %d %s %d\n",
				 cur_freq / 100.0, get_rssi(), cur_route, cur_vol);
		} else {
			snprintf(resp, sizeof(resp), "STOPPED %.1f %s %d\n",
				 cur_freq / 100.0, cur_route, cur_vol);
		}
	} else if (strncasecmp(buf, "START", 5) == 0) {
		float f = 0;
		if (sscanf(buf + 5, "%f", &f) == 1 && f >= 60.0 && f <= 110.0) {
			cur_freq = (int)(f * 100 + 0.5);
		}
		if (start_radio(cur_freq) == 0) {
			snprintf(resp, sizeof(resp), "OK %.1f %d\n", cur_freq / 100.0, get_rssi());
		} else {
			snprintf(resp, sizeof(resp), "ERROR Cannot start radio\n");
		}
	} else if (strcasecmp(buf, "STOP") == 0) {
		stop_radio();
		snprintf(resp, sizeof(resp), "OK STOPPED\n");
	} else if (strncasecmp(buf, "TUNE", 4) == 0) {
		float f = 0;
		if (sscanf(buf + 4, "%f", &f) == 1 && f >= 60.0 && f <= 110.0) {
			int target = (int)(f * 100 + 0.5);
			if (tune_radio(target) == 0) {
				snprintf(resp, sizeof(resp), "OK %.1f %d\n", cur_freq / 100.0, get_rssi());
			} else {
				snprintf(resp, sizeof(resp), "ERROR Cannot tune\n");
			}
		} else {
			snprintf(resp, sizeof(resp), "ERROR Invalid frequency\n");
		}
	} else if (strncasecmp(buf, "SEEK", 4) == 0) {
		int dir = 1;
		if (strstr(buf, "DOWN") || strstr(buf, "down")) dir = -1;
		if (seek_radio(dir) == 0) {
			snprintf(resp, sizeof(resp), "OK %.1f %d\n", cur_freq / 100.0, get_rssi());
		} else {
			snprintf(resp, sizeof(resp), "ERROR Seek failed\n");
		}
	} else if (strcasecmp(buf, "SCAN") == 0) {
		perform_scan(resp, sizeof(resp));
	} else if (strncasecmp(buf, "VOL", 3) == 0) {
		int v = 75;
		if (sscanf(buf + 3, "%d", &v) == 1) {
			if (v < 0) v = 0;
			if (v > 100) v = 100;
			cur_vol = v;
			apply_mixer_settings();
			snprintf(resp, sizeof(resp), "OK VOL %d\n", cur_vol);
		} else {
			snprintf(resp, sizeof(resp), "ERROR Invalid volume\n");
		}
	} else if (strncasecmp(buf, "ROUTE", 5) == 0) {
		if (strstr(buf, "speaker")) {
			strncpy(cur_route, "speaker", sizeof(cur_route) - 1);
		} else {
			strncpy(cur_route, "headphone", sizeof(cur_route) - 1);
		}
		apply_mixer_settings();
		snprintf(resp, sizeof(resp), "OK ROUTE %s\n", cur_route);
	} else if (strcasecmp(buf, "QUIT") == 0 || strcasecmp(buf, "EXIT") == 0) {
		stop_radio();
		keep_running = 0;
		snprintf(resp, sizeof(resp), "OK BYE\n");
	} else {
		snprintf(resp, sizeof(resp), "ERROR Unknown command\n");
	}

	write(client_fd, resp, strlen(resp));
	close(client_fd);
}

static int send_daemon_cmd(const char *cmd) {
	signal(SIGPIPE, SIG_IGN);
	int s = socket(AF_UNIX, SOCK_STREAM, 0);
	if (s < 0) return -1;

	struct sockaddr_un addr;
	memset(&addr, 0, sizeof(addr));
	addr.sun_family = AF_UNIX;
	strncpy(addr.sun_path, SOCK_PATH, sizeof(addr.sun_path) - 1);

	if (connect(s, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
		close(s);
		return -1;
	}

	char out[512];
	snprintf(out, sizeof(out), "%s\n", cmd);
	send(s, out, strlen(out), MSG_NOSIGNAL);

	char buf[8192];
	memset(buf, 0, sizeof(buf));
	int n = read(s, buf, sizeof(buf) - 1);
	if (n > 0) {
		printf("%s", buf);
	}
	close(s);
	return 0;
}

int main(int argc, char **argv) {
	signal(SIGPIPE, SIG_IGN);
	if (argc > 1) {
		if (strcmp(argv[1], "--foreground") != 0 && strcmp(argv[1], "-f") != 0) {
			char fullcmd[256] = {0};
			for (int i = 1; i < argc; i++) {
				if (i > 1) strcat(fullcmd, " ");
				strcat(fullcmd, argv[i]);
			}
			if (send_daemon_cmd(fullcmd) == 0) {
				return 0;
			}
			if (strcasecmp(argv[1], "start") != 0 && strcasecmp(argv[1], "play") != 0 && strcmp(argv[1], "--daemon") != 0) {
				printf("Radio daemon is not running.\n");
				return 1;
			}
		}
	}

	// Daemonize into background unless --foreground is passed
	if (argc <= 1 || (strcmp(argv[1], "--foreground") != 0 && strcmp(argv[1], "-f") != 0)) {
		if (daemon(0, 0) < 0) {
			perror("daemon");
			return 1;
		}
	}

	unlink(SOCK_PATH);
	int server_fd = socket(AF_UNIX, SOCK_STREAM, 0);
	if (server_fd < 0) { perror("socket"); return 1; }

	struct sockaddr_un addr;
	memset(&addr, 0, sizeof(addr));
	addr.sun_family = AF_UNIX;
	strncpy(addr.sun_path, SOCK_PATH, sizeof(addr.sun_path) - 1);

	if (bind(server_fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
		perror("bind");
		close(server_fd);
		return 1;
	}
	chmod(SOCK_PATH, 0666);
	listen(server_fd, 5);

	FILE *fp = fopen(PID_PATH, "w");
	if (fp) {
		fprintf(fp, "%d\n", getpid());
		fclose(fp);
	}

	signal(SIGINT, sig_handler);
	signal(SIGTERM, sig_handler);
	signal(SIGPIPE, SIG_IGN);

	if (argc > 2 && (strcmp(argv[1], "start") == 0 || strcmp(argv[1], "play") == 0)) {
		float f = atof(argv[2]);
		if (f >= 60.0 && f <= 110.0) cur_freq = (int)(f * 100 + 0.5);
		start_radio(cur_freq);
	}

	while (keep_running) {
		fd_set read_fds;
		FD_ZERO(&read_fds);
		FD_SET(server_fd, &read_fds);

		struct timeval tv = { .tv_sec = 1, .tv_usec = 0 };
		int ret = select(server_fd + 1, &read_fds, NULL, NULL, &tv);
		if (ret > 0 && FD_ISSET(server_fd, &read_fds)) {
			int client_fd = accept(server_fd, NULL, NULL);
			if (client_fd >= 0) {
				handle_client(client_fd);
			}
		}
	}

	stop_radio();
	close(server_fd);
	unlink(SOCK_PATH);
	unlink(PID_PATH);
	return 0;
}
