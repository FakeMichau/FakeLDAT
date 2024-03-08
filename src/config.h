#include "types.h"

#define BUTTON_PIN       13
#define AUTO_PIN         15
#define SENSOR_PIN       26
#define TRIGGER_ON_PRESS true  // as opposed to on release
#define KEY_TO_PRESS     0x78  // ASCII
#define MOUSE_TO_PRESS   MOUSE_LEFT
#define HISTORY_SIZE     150
#define THRESHOLD        0.95
#define INTERVAL_US      500   // 1000 -> 1000Hz, 2000 -> 500Hz

ReportMode  report_mode  = COMBINED;
TriggerMode trigger_mode = MOUSE;
