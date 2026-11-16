#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <linux/input.h>

#define ACCDET_PATH "/dev/input/by-path/platform-mt635x-accdet-event"

static int get_jack_state(int fd) {
    unsigned long sw = 0;
    if (ioctl(fd, EVIOCGSW(sizeof(sw)), &sw) < 0) {
        return -1;
    }
    int hp = !!(sw & (1UL << SW_HEADPHONE_INSERT));
    int mic = !!(sw & (1UL << SW_MICROPHONE_INSERT));
    int line = !!(sw & (1UL << SW_LINEOUT_INSERT));
    return (hp || mic || line);
}

int main(int argc, char **argv) {
    while (1) {
        int fd = open(ACCDET_PATH, O_RDONLY);
        if (fd < 0) {
            sleep(2);
            continue;
        }

        int current_state = get_jack_state(fd);
        int last_state = current_state;

        /* Apply initial route on daemon start */
        if (current_state == 1) {
            system("/usr/bin/sxmo_audio.sh route headphones");
        } else if (current_state == 0) {
            system("/usr/bin/sxmo_audio.sh route speaker");
        }

        struct input_event ev;
        while (read(fd, &ev, sizeof(ev)) > 0) {
            if (ev.type == EV_SW) {
                if (ev.code == SW_HEADPHONE_INSERT ||
                    ev.code == SW_MICROPHONE_INSERT ||
                    ev.code == SW_LINEOUT_INSERT) {

                    /* Debounce delay (100ms) for jack contact settling */
                    usleep(100000);
                    current_state = get_jack_state(fd);
                    if (current_state >= 0 && current_state != last_state) {
                        last_state = current_state;
                        if (current_state == 1) {
                            system("/usr/bin/sxmo_audio.sh route headphones");
                        } else {
                            system("/usr/bin/sxmo_audio.sh route speaker");
                        }
                    }
                }
            }
        }

        close(fd);
        sleep(1);
    }
    return 0;
}
