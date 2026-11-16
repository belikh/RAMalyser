"""Mechanical interface shared by the boards and the enclosure (millimetres).

Origin is the base board's top-left corner seen from above; x to the right,
y towards the front (the user). pcb/build.py checks the boards against
these numbers, and enclosure/enclosure.py builds the case from them.

The adapter is a cartridge: it lies flat, 5 mm above the base board, and
slides in from the rear on two rails until the gold fingers on its front
edge seat in a right-angle card-edge socket under the console.
"""
HW_REV = "A"                         # hardware revision, printed on every board
COPYRIGHT = "© 2026 Jupiter Belic"  # printed on every board under the revision
BASE_SIZE = (160.0, 150.0)
PCB_THICKNESS = 1.6
# M3 holes in the base board. The first four are under the cartridge bay and
# also hold the rails; the last two are under the console.
BASE_HOLES = [(5.0, 10.0), (155.0, 10.0), (5.0, 92.0), (155.0, 92.0),
              (5.0, 145.0), (155.0, 145.0)]

# ---- card-edge slot (Samtec PCIE-164-02-x-D-RA, right angle, through hole)
SLOT_PIN1 = (8.0, 118.55)            # x of position 1, y of the first row of holes; positions run +x
SLOT_MOUTH_Y = 100.0                 # plane of the socket mouth; the cartridge stops here
SLOT_BODY = (6.0, 100.0, 88.0, 19.6) # x, y, w, h of the socket body on the board
SLOT_HEIGHT = 11.05                  # socket body height above the board
CARD_CENTRE_HEIGHT = 5.8             # card mid-plane above the base board
ADAPTER_STACK = CARD_CENTRE_HEIGHT - PCB_THICKNESS / 2   # base top to cartridge underside

# ---- cartridge (adapter board)
ADAPTER_ORIGIN = (5.0, 0.0)          # adapter board corner in base coordinates when seated
ADAPTER_SIZE = (150.0, 100.0)        # body, without the tongue
TONGUE = (2.35, 84.3, 8.4)           # x offset on the adapter, width, length beyond the body
TONGUE_NOTCH = (14.5, 1.9)           # key notch: x of its centre on the adapter, width
EDGE_PIN1 = (3.0, 104.9)             # contact B1/A1 pad centre, adapter coordinates
ADAPTER_HOLES = [(5.0, 5.0), (145.0, 5.0), (5.0, 93.0), (145.0, 93.0)]
# Areas the sockets and ZIF levers need open in the cartridge shell: a list
# of rectangles (x, y, w, h) in adapter coordinates.
ADAPTER_WINDOWS = {
    "simm": [(11.0, 6.0, 128.0, 33.0)],    # 72-pin, 30-pin and SIPP sockets
    # five ZIF sockets and the ZIP strips; narrower at the rear to leave the
    # corner screw posts standing
    "dip": [(5.5, 9.5, 139.0, 51.0), (9.5, 2.5, 131.0, 7.2)],
}
ADAPTER_PART_HEIGHT = 2.2            # tallest SMD part under the shell
RAIL_KEEPOUT = 9.5                   # base parts stay this far from the left/right edges
                                     # of the bay (the rails sit there)

USB_C = (80.0, 0.0)                  # centre of the receptacle mouth, rear edge
USB_C_SIZE = (9.4, 3.4)              # mouth width, height above the board
LCD_HEADER = (102.0, 110.0)          # pin 1 of the 1x8 display header (pins run +y)
LCD_MODULE = (98.0, 107.0, 56.0, 34.0)    # x, y, w, h of the display module outline
LCD_WINDOW = (104.0, 109.0, 42.0, 30.0)   # visible area
LCD_HEIGHT = 13.5                    # top of the display glass above the board
KEYS = [(20.0 + 12.0 * i, 138.0) for i in range(6)]
KEY_NAMES = ["ESC", "F1", "F2", "F3", "F4", "F5"]
KEY_ACTUATOR_HEIGHT = 5.0            # switch actuator top above the board
LEDS = [(55.0 + 8.0 * i, 126.0) for i in range(3)]
BUZZER = (10.0, 128.0)
MAX_HEIGHT_UNDER_ADAPTER = 4.5       # tallest part allowed in the cartridge bay

# ---- optional Wi-Fi module: Seeed XIAO (ESP32-C6 or any other XIAO), on the
# BACK of the base board on plain 2.54 mm pin headers, antenna end at the
# rear edge. Two rows of 7 holes, 15.24 mm apart. Coordinates are seen from
# the top of the base board, so the rows are mirrored against the module's
# own top view.
XIAO_PIN1 = (112.38, 18.24)          # D0 (USB end of the module); D0..D6 run towards the rear
XIAO_ROW_SPACING = 15.24             # D7..5V row sits this far to the right (+x)
XIAO_BODY = (111.1, 0.1, 17.8, 21.0) # x, y, w, h of the module outline
XIAO_ANTENNA = (113.8, 0.0, 12.4, 5.0)    # no copper on any layer here
XIAO_HEIGHT = 6.7                    # module underside to its tallest part, on pin headers
XIAO_WINDOW = (109.5, 0.5, 21.0, 48.0)    # opening in the tray floor: module plus a USB-C plug
