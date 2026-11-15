// ECP5 top level: 25 MHz oscillator -> 162.5 MHz core clock (3.077 ns tick), SPI register
// interface from the RP2350, 116 DDR bus cells with fine input delay.
`include "ramspec.vh"

module top_ecp5 (
    input  wire        clk25,
    input  wire        spi_sck,
    input  wire        spi_cs_n,
    input  wire        spi_mosi,
    output wire        spi_miso,
    input  wire        dut_pg,
    output wire        busy,
    output wire        pll_lock,
    inout  wire [`N_IO-1:0] io
);
    wire clk, locked;
    // 25 MHz / 2 * 13 = 162.5 MHz (PFD 12.5 MHz, VCO 650 MHz).
    (* FREQUENCY_PIN_CLKI="25" *)
    (* FREQUENCY_PIN_CLKOP="162.5" *)
    (* ICP_CURRENT="12" *) (* LPF_RESISTOR="8" *) (* MFG_ENABLE_FILTEROPAMP="1" *)
    (* MFG_GMCREF_SEL="2" *)
    EHXPLLL #(
        .PLLRST_ENA("DISABLED"), .INTFB_WAKE("DISABLED"), .STDBY_ENABLE("DISABLED"),
        .DPHASE_SOURCE("DISABLED"), .OUTDIVIDER_MUXA("DIVA"), .OUTDIVIDER_MUXB("DIVB"),
        .OUTDIVIDER_MUXC("DIVC"), .OUTDIVIDER_MUXD("DIVD"),
        .CLKI_DIV(2), .CLKOP_ENABLE("ENABLED"), .CLKOP_DIV(4), .CLKOP_CPHASE(3),
        .CLKOP_FPHASE(0), .FEEDBK_PATH("CLKOP"), .CLKFB_DIV(13)
    ) u_pll (
        .RST(1'b0), .STDBY(1'b0), .CLKI(clk25), .CLKOP(clk), .CLKFB(clk),
        .CLKINTFB(), .PHASESEL0(1'b0), .PHASESEL1(1'b0), .PHASEDIR(1'b1),
        .PHASESTEP(1'b1), .PHASELOADREG(1'b1), .PLLWAKESYNC(1'b0),
        .ENCLKOP(1'b0), .LOCK(locked)
    );
    assign pll_lock = locked;

    reg [7:0] rst_sr = 8'hFF;
    always @(posedge clk) rst_sr <= {rst_sr[6:0], !locked};
    wire rst = rst_sr[7];

    wire        reg_we;
    wire [6:0]  reg_addr;
    wire [31:0] reg_wdata, reg_rdata;
    wire [`N_IO-1:0] d0, d1, oe, q0, q1;
    wire [6:0]  fine;
    reg         pg_s0, pg_s1;
    always @(posedge clk) begin pg_s0 <= dut_pg; pg_s1 <= pg_s0; end

    spi_regs u_spi (.clk(clk), .rst(rst), .sck(spi_sck), .cs_n(spi_cs_n),
                    .mosi(spi_mosi), .miso(spi_miso), .reg_we(reg_we),
                    .reg_addr(reg_addr), .reg_wdata(reg_wdata), .reg_rdata(reg_rdata));

    tester_core #(.TICK_PS(3077)) u_core (.clk(clk), .rst(rst), .reg_we(reg_we), .reg_addr(reg_addr),
                        .reg_wdata(reg_wdata), .reg_rdata(reg_rdata),
                        .io_d0(d0), .io_d1(d1), .io_oe(oe), .io_q0(q0), .io_q1(q1),
                        .dut_pg(pg_s1), .fine(fine), .busy(busy));

    // Fine-delay stepper: the ECP5 delay cell is stepped, not loaded with a
    // value. On any change reload (back to zero) and step up to the target.
    reg [6:0] cur, tgt;
    reg       f_load, f_move;
    reg [1:0] fs;
    always @(posedge clk) begin
        f_load <= 1'b0;
        if (rst) begin cur <= 0; tgt <= 0; fs <= 0; f_move <= 0; end
        else case (fs)
        2'd0: if (fine != tgt) begin tgt <= fine; cur <= 0; f_load <= 1'b1; fs <= 2'd1; end
        2'd1: if (cur == tgt) fs <= 2'd0; else begin f_move <= 1'b1; fs <= 2'd2; end
        2'd2: begin f_move <= 1'b0; cur <= cur + 7'd1; fs <= 2'd1; end
        default: fs <= 2'd0;
        endcase
    end

    genvar g;
    generate for (g = 0; g < `N_IO; g = g + 1) begin : cells
        io_ddr #(.DELAY(1)) u_io (.clk(clk), .d0(d0[g]), .d1(d1[g]), .oe(oe[g]),
            .q0(q0[g]), .q1(q1[g]), .fine(fine), .fine_load(f_load),
            .fine_move(f_move), .pad(io[g]));
    end endgenerate
endmodule
