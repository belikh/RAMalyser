// Bus I/O cells: DDR output with tristate, DDR input with a fine input delay.
// SIM selects behavioural models; otherwise Lattice ECP5 primitives are used.
`timescale 1ns/1ps

module io_ddr #(parameter DELAY = 1) (
    input  wire       clk,
    input  wire       d0,       // first half of the clk cycle
    input  wire       d1,       // second half
    input  wire       oe,
    output wire       q0,       // sampled on rising edge
    output wire       q1,       // sampled on falling edge, re-registered
    input  wire [6:0] fine,     // input delay, ~25 ps per step (static in sim)
    input  wire       fine_load,
    input  wire       fine_move,
    inout  wire       pad
);
`ifdef SIM
    reg r0, r1, roe;
    always @(posedge clk) begin r0 <= d0; r1 <= d1; roe <= oe; end
    wire o = clk ? r0 : r1;
    assign pad = roe ? o : 1'bz;
    wire #(fine * 0.025) pad_d = pad;
    reg s0, s1n, s1;
    always @(posedge clk) begin s0 <= pad_d; s1 <= s1n; end
    always @(negedge clk) s1n <= pad_d;
    assign q0 = s0;
    assign q1 = s1;
`else
    wire o, i, i_d;
    reg  rt = 1'b1, ld_n = 1'b1, mv = 1'b0;   // local copies keep fan-out paths short
    always @(posedge clk) begin rt <= !oe; ld_n <= !fine_load; mv <= fine_move; end
    ODDRX1F u_o (.SCLK(clk), .RST(1'b0), .D0(d0), .D1(d1), .Q(o));
    BB u_bb (.I(o), .T(rt), .O(i), .B(pad));
    generate if (DELAY) begin : g_dly
        DELAYF #(.DEL_MODE("USER_DEFINED"), .DEL_VALUE(0)) u_d (
            .A(i), .LOADN(ld_n), .MOVE(mv), .DIRECTION(1'b0),
            .Z(i_d), .CFLAG());
    end else begin : g_nodly
        assign i_d = i;
    end endgenerate
    IDDRX1F u_i (.SCLK(clk), .RST(1'b0), .D(i_d), .Q0(q0), .Q1(q1));
`endif
endmodule
