// SPDX-License-Identifier: GPL-3.0-only
// meet2-exercise: drive one OBSBOT libdev.so API per invocation, so a
// dumpcap on usbmon<bus> sees exactly the bytes for that one method.
//
// Build (needs the OBSBOT public SDK headers + libdev.so):
//   g++ -std=c++17 -O2 \
//     -I$OBSBOT_SDK/include \
//     -L$OBSBOT_SDK/lib -Wl,-rpath,$OBSBOT_SDK/lib \
//     meet2_exercise.cpp -ldev -lpthread -o meet2-exercise
//
// $OBSBOT_SDK points at an unpacked sdk/v1.0.2/ tree (e.g. from
// aaronsb/obsbot-camera-control/sdk/v1.0.2/). See the sourcing rule:
// the public C++ headers and the shipped libdev.so are permitted
// inputs; their internal source is not.

#include <chrono>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <dev/devs.hpp>
#include <iostream>
#include <string>
#include <thread>

using namespace std;
using namespace std::chrono;

static int usage() {
    cerr << "usage: meet2-exercise --<api> <value>\n"
            "  --set-media-mode  <0|1|2>          (Normal / Background / AutoFrame)\n"
            "  --set-wdr         <0|1>            (off / DOL2-to-1)\n"
            "  --set-fov         <0|1|2>          (86 / 78 / 65)\n"
            "  --set-face-ae     <0|1>\n"
            "  --set-face-focus  <0|1>\n";
    return 2;
}

int main(int argc, char **argv) {
    if (argc != 3)
        return usage();

    string api = argv[1];
    int value = atoi(argv[2]);

    bool connected = false;
    auto on_change = [&connected](string sn, bool c, void *) {
        if (c)
            connected = true;
    };
    Devices::get().setDevChangedCallback(on_change, nullptr);
    Devices::get().setEnableMdnsScan(false);

    cerr << "waiting for OBSBOT camera...\n";
    for (int i = 0; i < 100 && !connected; ++i)
        this_thread::sleep_for(milliseconds(100));
    if (!connected) {
        cerr << "no camera detected within 10s\n";
        return 1;
    }
    this_thread::sleep_for(milliseconds(300));

    auto list = Devices::get().getDevList();
    if (list.empty()) {
        cerr << "device list empty\n";
        return 1;
    }
    auto dev = list.front();
    cerr << "device: " << dev->devSn() << " fw=" << dev->devVersion() << "\n";

    int32_t ret = -1;
    if (api == "--set-media-mode")
        ret = dev->cameraSetMediaModeU(static_cast<Device::MediaMode>(value));
    else if (api == "--set-wdr")
        ret = dev->cameraSetWdrR(static_cast<Device::DevWdrMode>(value));
    else if (api == "--set-fov")
        ret = dev->cameraSetFovU(static_cast<Device::FovType>(value));
    else if (api == "--set-face-ae")
        ret = dev->cameraSetFaceAER(value);
    else if (api == "--set-face-focus")
        ret = dev->cameraSetFaceFocusR(value != 0);
    else
        return usage();

    cerr << api << " " << value << " -> " << ret << "\n";
    return ret == 0 ? 0 : 1;
}
