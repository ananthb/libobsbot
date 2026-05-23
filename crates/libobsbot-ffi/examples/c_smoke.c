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

    /* Drain whatever the watcher emitted on startup (one DeviceAdded
     * per connected camera). With no hardware the queue is empty and
     * try_recv returns TIMEOUT. */
    ObsbotEvent ev;
    for (int i = 0; i < 16; i++) {
        int rc = obsbot_devices_poll_event(d, &ev, 0);
        if (rc != OBSBOT_OK) break;
        printf("event kind=%d serial=\"%s\"\n", ev.kind, ev.serial);
    }

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
    EXPECT(obsbot_device_set_audio_agc(NULL, 1),     OBSBOT_ERR_NOT_FOUND, "set_audio_agc");
    EXPECT(obsbot_device_set_flip_horizontal(NULL, 0), OBSBOT_ERR_NOT_FOUND, "set_flip_horizontal");
    EXPECT(obsbot_device_set_portrait(NULL, 0),      OBSBOT_ERR_NOT_FOUND, "set_portrait");
    EXPECT(obsbot_device_set_led(NULL, 0),           OBSBOT_ERR_NOT_FOUND, "set_led");
    EXPECT(obsbot_device_set_hue(NULL, 0),           OBSBOT_ERR_NOT_FOUND, "set_hue");
    EXPECT(obsbot_device_set_sharpness(NULL, 0),     OBSBOT_ERR_NOT_FOUND, "set_sharpness");
    EXPECT(obsbot_device_set_gain(NULL, 0),          OBSBOT_ERR_NOT_FOUND, "set_gain");
    EXPECT(obsbot_device_set_backlight_compensation(NULL, 0), OBSBOT_ERR_NOT_FOUND, "set_backlight");
    EXPECT(obsbot_device_set_anti_flicker(NULL, 1),  OBSBOT_ERR_NOT_FOUND, "set_anti_flicker");
    EXPECT(obsbot_device_set_anti_flicker(NULL, 99), OBSBOT_ERR_OUT_OF_RANGE, "set_anti_flicker range");
    EXPECT(obsbot_device_set_auto_focus(NULL, 1),    OBSBOT_ERR_NOT_FOUND, "set_auto_focus");
    EXPECT(obsbot_device_set_ae_mode(NULL, 0),       OBSBOT_ERR_NOT_FOUND, "set_ae_mode");
    EXPECT(obsbot_device_set_ae_mode(NULL, 99),      OBSBOT_ERR_OUT_OF_RANGE, "set_ae_mode range");
    EXPECT(obsbot_device_set_ae_lock(NULL, 1),       OBSBOT_ERR_NOT_FOUND, "set_ae_lock");
    EXPECT(obsbot_device_set_exposure_time(NULL, 100), OBSBOT_ERR_NOT_FOUND, "set_exposure_time");
    EXPECT(obsbot_device_set_status_cadence(NULL, 0), OBSBOT_ERR_NOT_FOUND, "set_status_cadence");

    /* Out-of-range enum values must fail without touching device state. */
    EXPECT(obsbot_device_set_wdr(NULL, 99),          OBSBOT_ERR_OUT_OF_RANGE, "set_wdr range");
    EXPECT(obsbot_device_set_ai_mode(NULL, 99),      OBSBOT_ERR_OUT_OF_RANGE, "set_ai_mode range");

    int32_t value = 0;
    EXPECT(obsbot_device_brightness(NULL, &value),   OBSBOT_ERR_NOT_FOUND, "brightness");
    EXPECT(obsbot_device_brightness(NULL, NULL),     OBSBOT_ERR_NOT_FOUND, "brightness null out");
    EXPECT(obsbot_device_contrast(NULL, &value),     OBSBOT_ERR_NOT_FOUND, "contrast");
    EXPECT(obsbot_device_saturation(NULL, &value),   OBSBOT_ERR_NOT_FOUND, "saturation");

    float fval = 0.f;
    EXPECT(obsbot_device_zoom(NULL, &fval),          OBSBOT_ERR_NOT_FOUND, "zoom");
    EXPECT(obsbot_device_focus(NULL, &fval),         OBSBOT_ERR_NOT_FOUND, "focus");
    EXPECT(obsbot_device_pan_tilt(NULL, &fval, &fval), OBSBOT_ERR_NOT_FOUND, "pan_tilt");

    ObsbotStatus snap;
    EXPECT(obsbot_device_status(NULL, &snap),        OBSBOT_ERR_NOT_FOUND, "status");
    EXPECT(obsbot_device_status((ObsbotDevice *)0x1, NULL), OBSBOT_ERR_NOT_FOUND, "status null out");

    int wb_mode = 0;
    uint16_t kelvin = 0;
    EXPECT(obsbot_device_white_balance(NULL, &wb_mode, &kelvin), OBSBOT_ERR_NOT_FOUND, "white_balance");

    int xu = 0;
    EXPECT(obsbot_device_wdr(NULL, &xu),     OBSBOT_ERR_NOT_FOUND, "wdr");
    EXPECT(obsbot_device_face_ae(NULL, &xu), OBSBOT_ERR_NOT_FOUND, "face_ae");
    EXPECT(obsbot_device_ai_mode(NULL, &xu), OBSBOT_ERR_NOT_FOUND, "ai_mode");

    EXPECT(obsbot_devices_poll_event(NULL, &ev, 0),  OBSBOT_ERR_NOT_FOUND, "poll_event null handle");
    EXPECT(obsbot_devices_poll_event(d, NULL, 0),    OBSBOT_ERR_NOT_FOUND, "poll_event null out");

    char buf[64];
    EXPECT(obsbot_device_firmware(NULL, buf, sizeof buf), OBSBOT_ERR_NOT_FOUND, "firmware");
    EXPECT(obsbot_device_serial(NULL, buf, sizeof buf),   OBSBOT_ERR_NOT_FOUND, "serial");

    obsbot_devices_free(d);
    printf("c_smoke OK\n");
    return 0;
}
