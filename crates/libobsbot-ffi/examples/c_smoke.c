/* SPDX-License-Identifier: GPL-3.0-only */
/* C smoke test - links against liblibobsbot.{so,dylib,dll}.
 *
 * Build (Linux/macOS):
 *   cargo build --release -p libobsbot-ffi
 *   cc examples/c_smoke.c -I include -L target/release -llibobsbot \
 *      -Wl,-rpath,$PWD/target/release -o c_smoke
 *   ./c_smoke
 *
 * Designed to be safe to run without hardware: every call uses a NULL
 * device handle and checks for OBSBOT_ERR_NOT_FOUND. The Garnix
 * valgrind-c-smoke check runs this binary inside valgrind to catch any
 * leaks in the FFI argument-handling paths.
 */

#include <stdio.h>
#include "libobsbot.h"

#define EXPECT(actual, expected, label)                                        \
    do {                                                                       \
        if ((actual) != (expected)) {                                          \
            fprintf(stderr, "%s: expected %d, got %d\n", (label), (expected),  \
                    (actual));                                                 \
            return 1;                                                          \
        }                                                                      \
    } while (0)

int main(void) {
    printf("libobsbot version: %s\n", obsbot_version());

    ObsbotDevices *d = obsbot_devices_new();
    if (!d) {
        fprintf(stderr, "obsbot_devices_new failed\n");
        return 1;
    }

    int n = obsbot_devices_count(d);
    printf("connected obsbot cameras: %d\n", n);

    /* Every device setter must report OBSBOT_ERR_NOT_FOUND on NULL.
     * This covers the FFI argument-handling paths under valgrind
     * without needing a real camera. */
    EXPECT(obsbot_device_set_brightness(NULL, 0),    OBSBOT_ERR_NOT_FOUND, "set_brightness");
    EXPECT(obsbot_device_set_contrast(NULL, 0),      OBSBOT_ERR_NOT_FOUND, "set_contrast");
    EXPECT(obsbot_device_set_saturation(NULL, 0),    OBSBOT_ERR_NOT_FOUND, "set_saturation");
    EXPECT(obsbot_device_set_pan_tilt(NULL, 0, 0),   OBSBOT_ERR_NOT_FOUND, "set_pan_tilt");
    EXPECT(obsbot_device_set_zoom(NULL, 0),          OBSBOT_ERR_NOT_FOUND, "set_zoom");
    EXPECT(obsbot_device_set_focus(NULL, 0),         OBSBOT_ERR_NOT_FOUND, "set_focus");
    EXPECT(obsbot_device_set_white_balance(NULL, 0, 6500), OBSBOT_ERR_NOT_FOUND, "set_white_balance");
    EXPECT(obsbot_device_set_wdr(NULL, 0),           OBSBOT_ERR_NOT_FOUND, "set_wdr");
    EXPECT(obsbot_device_set_fov(NULL, 0),           OBSBOT_ERR_NOT_FOUND, "set_fov");
    EXPECT(obsbot_device_set_face_ae(NULL, 1),       OBSBOT_ERR_NOT_FOUND, "set_face_ae");
    EXPECT(obsbot_device_set_face_focus(NULL, 1),    OBSBOT_ERR_NOT_FOUND, "set_face_focus");
    EXPECT(obsbot_device_set_media_mode(NULL, 0),    OBSBOT_ERR_NOT_FOUND, "set_media_mode");
    EXPECT(obsbot_device_set_auto_framing(NULL, 0),  OBSBOT_ERR_NOT_FOUND, "set_auto_framing");
    EXPECT(obsbot_device_set_ai_mode(NULL, 0),       OBSBOT_ERR_NOT_FOUND, "set_ai_mode");
    EXPECT(obsbot_device_set_status_cadence(NULL, 0), OBSBOT_ERR_NOT_FOUND, "set_status_cadence");

    /* Out-of-range enum values must fail without touching device state. */
    EXPECT(obsbot_device_set_wdr(NULL, 99),          OBSBOT_ERR_OUT_OF_RANGE, "set_wdr range");
    EXPECT(obsbot_device_set_ai_mode(NULL, 99),      OBSBOT_ERR_OUT_OF_RANGE, "set_ai_mode range");

    int32_t value = 0;
    EXPECT(obsbot_device_brightness(NULL, &value),   OBSBOT_ERR_NOT_FOUND, "brightness");
    EXPECT(obsbot_device_brightness(NULL, NULL),     OBSBOT_ERR_NOT_FOUND, "brightness null out");

    char buf[64];
    EXPECT(obsbot_device_firmware(NULL, buf, sizeof buf), OBSBOT_ERR_NOT_FOUND, "firmware");
    EXPECT(obsbot_device_serial(NULL, buf, sizeof buf),   OBSBOT_ERR_NOT_FOUND, "serial");

    obsbot_devices_free(d);
    printf("c_smoke OK\n");
    return 0;
}
