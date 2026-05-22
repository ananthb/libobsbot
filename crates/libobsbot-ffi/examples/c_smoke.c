/* SPDX-License-Identifier: GPL-3.0-only */
/* C smoke test - links against liblibobsbot.{so,dylib,dll}.
 *
 * Build (Linux/macOS):
 *   cargo build --release -p libobsbot-ffi
 *   cc examples/c_smoke.c -I include -L target/release -llibobsbot \
 *      -Wl,-rpath,$PWD/target/release -o c_smoke
 *   ./c_smoke
 */

#include <stdio.h>
#include "libobsbot.h"

int main(void) {
    printf("libobsbot version: %s\n", obsbot_version());

    ObsbotDevices *d = obsbot_devices_new();
    if (!d) {
        fprintf(stderr, "obsbot_devices_new failed\n");
        return 1;
    }

    int n = obsbot_devices_count(d);
    printf("connected obsbot cameras: %d\n", n);

    obsbot_devices_free(d);
    return 0;
}
