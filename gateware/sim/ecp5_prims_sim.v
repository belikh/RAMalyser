// Simulation stand-ins for the four ECP5 primitives the open toolchain ships
// only as black boxes: EHXPLLL, ODDRX1F, IDDRX1F and DELAYF. Used by the
// gate-level run (run_gate.sh) with yosys's own models for everything else
// (LUT4, TRELLIS_FF, CCU2C, TRELLIS_IO, ...).
//
// These are written from the primitives' documented function, not taken
// from the vendor's library, and they carry the same latency as the
// behavioural pad cell in rtl/io_cells.v. A gate-level pass therefore says
// the synthesised logic and the top level behave like the source. It does
// not confirm the pad cells' real latency or the PLL's analogue behaviour.
`timescale 1ns/1ps

// Output DDR: D0 and D1 are taken on the rising edge; D0 goes out while the
// clock is high, D1 while it is low.
module ODDRX1F #(parameter GSR = "ENABLED") (
    input wire SCLK, RST, D0, D1, output wire Q);
    reg r0 = 1'b0, r1 = 1'b0;
    always @(posedge SCLK) if (RST) begin r0 <= 1'b0; r1 <= 1'b0; end
                           else begin r0 <= D0; r1 <= D1; end
    assign Q = SCLK ? r0 : r1;
endmodule

// Input DDR: Q0 is the pad at the rising edge; Q1 is the pad at the falling
// edge, handed over on the next rising edge.
module IDDRX1F #(parameter GSR = "ENABLED") (
    input wire D, SCLK, RST, output wire Q0, Q1);
    reg s0 = 1'b0, s1n = 1'b0, s1 = 1'b0;
    always @(posedge SCLK) if (RST) begin s0 <= 1'b0; s1 <= 1'b0; end
                           else begin s0 <= D; s1 <= s1n; end
    always @(negedge SCLK) s1n <= D;
    assign Q0 = s0;
    assign Q1 = s1;
endmodule

// Input delay: 25 ps per step, 0 to 127 steps. LOADN low returns it to
// DEL_VALUE; each rising edge of MOVE steps it up (DIRECTION = 0) or down.
module DELAYF #(parameter DEL_MODE = "USER_DEFINED", parameter DEL_VALUE = 0) (
    input wire A, LOADN, MOVE, DIRECTION, output wire Z, output wire CFLAG);
    reg [6:0] steps = DEL_VALUE;
    always @(negedge LOADN) steps <= DEL_VALUE;
    always @(posedge MOVE) if (LOADN) begin
        if (!DIRECTION && steps != 7'd127) steps <= steps + 7'd1;
        if (DIRECTION && steps != 7'd0)    steps <= steps - 7'd1;
    end
    assign CFLAG = DIRECTION ? (steps == 7'd0) : (steps == 7'd127);
    wire #(steps * 0.025) dly = A;
    assign Z = dly;
endmodule

// PLL: an ideal clock at f(CLKI) / CLKI_DIV * CLKFB_DIV (feedback from
// CLKOP), with LOCK after 16 input cycles. No VCO range, loop dynamics,
// jitter or phase settings; the other outputs are not driven.
module EHXPLLL #(
    parameter CLKI_DIV = 1, CLKFB_DIV = 1, CLKOP_DIV = 8, CLKOS_DIV = 8, CLKOS2_DIV = 8,
    CLKOS3_DIV = 8, CLKOP_ENABLE = "ENABLED", CLKOS_ENABLE = "DISABLED",
    CLKOS2_ENABLE = "DISABLED", CLKOS3_ENABLE = "DISABLED", CLKOP_CPHASE = 0,
    CLKOS_CPHASE = 0, CLKOS2_CPHASE = 0, CLKOS3_CPHASE = 0, CLKOP_FPHASE = 0,
    CLKOS_FPHASE = 0, CLKOS2_FPHASE = 0, CLKOS3_FPHASE = 0, FEEDBK_PATH = "CLKOP",
    CLKOP_TRIM_POL = "RISING", CLKOP_TRIM_DELAY = 0, CLKOS_TRIM_POL = "RISING",
    CLKOS_TRIM_DELAY = 0, OUTDIVIDER_MUXA = "DIVA", OUTDIVIDER_MUXB = "DIVB",
    OUTDIVIDER_MUXC = "DIVC", OUTDIVIDER_MUXD = "DIVD", PLL_LOCK_MODE = 0,
    PLL_LOCK_DELAY = 200, STDBY_ENABLE = "DISABLED", REFIN_RESET = "DISABLED",
    SYNC_ENABLE = "DISABLED", INT_LOCK_STICKY = "ENABLED", DPHASE_SOURCE = "DISABLED",
    PLLRST_ENA = "DISABLED", INTFB_WAKE = "DISABLED"
) (
    input wire CLKI, CLKFB, PHASESEL1, PHASESEL0, PHASEDIR, PHASESTEP, PHASELOADREG,
    input wire STDBY, PLLWAKESYNC, RST, ENCLKOP, ENCLKOS, ENCLKOS2, ENCLKOS3,
    output reg CLKOP, output wire CLKOS, CLKOS2, CLKOS3, output reg LOCK,
    output wire INTLOCK, REFCLK, CLKINTFB);
    realtime t_last, period;
    integer n;
    initial begin CLKOP = 1'b0; LOCK = 1'b0; n = 0; period = 0; t_last = 0; end
    always @(posedge CLKI) begin
        if (n > 0) period = ($realtime - t_last) * CLKI_DIV / CLKFB_DIV;
        t_last = $realtime;
        if (n < 16) n = n + 1; else LOCK <= 1'b1;
    end
    always begin
        if (period > 0) #(period / 2.0) CLKOP = ~CLKOP;
        else @(posedge CLKI);
    end
    assign CLKOS = 1'b0; assign CLKOS2 = 1'b0; assign CLKOS3 = 1'b0;
    assign INTLOCK = LOCK; assign REFCLK = CLKI; assign CLKINTFB = 1'b0;
endmodule
