// The synchronous part of the FPGA for the whole-chain simulation: the SPI
// register block and the tester core exactly as they are built for the
// ECP5, at the real tick (3.077 ns). The PLL and the DDR pad cells are left
// out; rt-cosim models the pads in software.
`include "ramspec.vh"

module cosim_top (
    input  wire        clk,
    input  wire        rst,
    input  wire        spi_sck,
    input  wire        spi_cs_n,
    input  wire        spi_mosi,
    output wire        spi_miso,
    input  wire        dut_pg,
    output wire        busy,
    output wire [6:0]  fine,
    output wire [`N_IO-1:0] io_d0,
    output wire [`N_IO-1:0] io_d1,
    output wire [`N_IO-1:0] io_oe,
    input  wire [`N_IO-1:0] io_q0,
    input  wire [`N_IO-1:0] io_q1
);
    wire        reg_we;
    wire [6:0]  reg_addr;
    wire [31:0] reg_wdata, reg_rdata;
    reg         pg_s0, pg_s1;
    always @(posedge clk) begin pg_s0 <= dut_pg; pg_s1 <= pg_s0; end

    spi_regs u_spi (.clk(clk), .rst(rst), .sck(spi_sck), .cs_n(spi_cs_n),
                    .mosi(spi_mosi), .miso(spi_miso), .reg_we(reg_we),
                    .reg_addr(reg_addr), .reg_wdata(reg_wdata), .reg_rdata(reg_rdata));
    tester_core #(.TICK_PS(3077)) u_core (.clk(clk), .rst(rst), .reg_we(reg_we),
                    .reg_addr(reg_addr), .reg_wdata(reg_wdata), .reg_rdata(reg_rdata),
                    .io_d0(io_d0), .io_d1(io_d1), .io_oe(io_oe), .io_q0(io_q0),
                    .io_q1(io_q1), .dut_pg(pg_s1), .fine(fine), .busy(busy));
endmodule
