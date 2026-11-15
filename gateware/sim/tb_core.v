// End-to-end simulation: SPI master -> spi_regs -> tester_core -> DDR cells
// -> modelled adapter buffers -> DRAM model with injected faults.
`timescale 1ns/1ps
`include "ramspec.vh"

module tb_core;
    parameter AW = 3;
    parameter DW = 8;
    parameter real T_RAC = 60.0;
    parameter real T_CAC = 15.0;
    parameter EDO = 0;
    parameter real T_RET = 0.0;
    parameter STUCK_EN = 0;
    parameter STUCK_ADDR = 37;
    parameter STUCK_BIT = 3;
    parameter STUCK_VAL = 0;
    parameter ADDR_STUCK_LINE = -1;
    parameter DQ_SHORT_A = -1;
    parameter DQ_SHORT_B = -1;
    parameter real T_DRV = 4.0;     // adapter drive buffer delay
    parameter real T_RX  = 5.0;     // adapter receive buffer delay
    parameter EXPECT_FAIL = 0;      // scenario expects the march to report errors
    parameter MODE = 0;             // 0 march, 1 speed sweep, 2 EDO detect, 3 retention

    reg clk = 0, rst = 1;
    always #2.5 clk = ~clk;         // 200 MHz, tick = 2.5 ns

    reg sck = 0, cs_n = 1, mosi = 0;
    wire miso;
    wire        reg_we;
    wire [6:0]  reg_addr;
    wire [31:0] reg_wdata, reg_rdata;
    wire [`N_IO-1:0] d0, d1, oe, q0, q1;
    wire [`N_IO-1:0] pad;
    wire [6:0] fine;

    spi_regs u_spi (.clk(clk), .rst(rst), .sck(sck), .cs_n(cs_n), .mosi(mosi),
                    .miso(miso), .reg_we(reg_we), .reg_addr(reg_addr),
                    .reg_wdata(reg_wdata), .reg_rdata(reg_rdata));
    tester_core u_core (.clk(clk), .rst(rst), .reg_we(reg_we), .reg_addr(reg_addr),
                        .reg_wdata(reg_wdata), .reg_rdata(reg_rdata),
                        .io_d0(d0), .io_d1(d1), .io_oe(oe), .io_q0(q0), .io_q1(q1),
                        .dut_pg(1'b1), .fine(fine), .busy());
    genvar g;
    generate for (g = 0; g < `N_IO; g = g + 1) begin : cells
        io_ddr u_io (.clk(clk), .d0(d0[g]), .d1(d1[g]), .oe(oe[g]), .q0(q0[g]),
                     .q1(q1[g]), .fine(fine), .fine_load(1'b0), .fine_move(1'b0),
                     .pad(pad[g]));
    end endgenerate

    // ---- adapter model: drive buffers, receive buffers, weak pull-ups ----
    wire [DW-1:0] dq_dut;                 // socket-side data bus
    wire [DW-1:0] dram_q;
    wire          dram_q_en;
    wire #(T_DRV) ras_n = pad[`IO_RAS0_N];
    wire #(T_DRV) cas_n = pad[`IO_CAS0_N];
    wire #(T_DRV) we_n  = pad[`IO_WE0_N];
    wire #(T_DRV) oe_n  = pad[`IO_OE0_N];
    wire [AW-1:0] #(T_DRV) ma = pad[`IO_MA0 +: AW];
    wire          drv_en = !pad[`IO_DQ_DRV_EN0_N];
    wire          rx_en  = !pad[`IO_DQ_RX_EN_N];
    wire [DW-1:0] #(T_DRV) dq_from_fpga = pad[DW-1:0];
    assign dq_dut = drv_en ? dq_from_fpga : {DW{1'bz}};
    assign dq_dut = dram_q_en ? dram_q : {DW{1'bz}};
    generate for (g = 0; g < DW; g = g + 1) begin : pulls
        pullup (weak1) (dq_dut[g]);       // PULL_A/PULL_B held high in this bench
    end endgenerate
    wire [DW-1:0] #(T_RX) dq_to_fpga = dq_dut;
    assign pad[DW-1:0] = rx_en ? dq_to_fpga : {DW{1'bz}};

    dram_model #(.AW(AW), .DW(DW), .T_RAC(T_RAC), .T_CAC(T_CAC), .EDO(EDO),
                 .T_RET(T_RET), .STUCK_EN(STUCK_EN), .STUCK_ADDR(STUCK_ADDR),
                 .STUCK_BIT(STUCK_BIT), .STUCK_VAL(STUCK_VAL),
                 .ADDR_STUCK_LINE(ADDR_STUCK_LINE), .DQ_SHORT_A(DQ_SHORT_A),
                 .DQ_SHORT_B(DQ_SHORT_B))
        u_dram (.ras_n(ras_n), .cas_n(cas_n), .we_n(we_n), .oe_n(oe_n), .a(ma),
                .d(dq_dut), .q(dram_q), .q_en(dram_q_en));

    // ------------------------------------------------------- SPI master
    reg [31:0] rd;
    task spi(input wr, input [6:0] a, input [31:0] wdata);
        reg [39:0] f; integer n;
        begin
            f = {wr, a, wdata}; rd = 0;
            cs_n = 0; #100;
            for (n = 39; n >= 0; n = n - 1) begin
                mosi = f[n]; #50; sck = 1;
                if (n < 32) rd[n] = miso;
                #50; sck = 0;
            end
            #100; cs_n = 1; #100;
        end
    endtask
    task wr(input [6:0] a, input [31:0] v); spi(1'b1, a, v); endtask
    task rdreg(input [6:0] a); spi(1'b0, a, 32'd0); endtask

    // one march element; ops packed as in register 0x0A
    task element(input dir, input [1:0] patmode, input [2:0] nops, input [11:0] ops);
        begin
            wr(7'h0A, {12'd0, ops, 2'b00, patmode, dir, nops});
            wr(7'h01, 32'h15);                    // start | rfsh_en | eng_en
            rdreg(7'h02);
            while (rd[0]) begin #2000; rdreg(7'h02); end
        end
    endtask

    localparam R0 = 2'b00, R1 = 2'b10, W0 = 2'b01, W1 = 2'b11;
    integer total_err;
    task acc; begin rdreg(7'h12); total_err = total_err + rd; end endtask

    // March C-: {any(w0); up(r0,w1); up(r1,w0); down(r0,w1); down(r1,w0); any(r0)}
    task march_c(input [1:0] patmode);
        begin
            element(0, patmode, 1, {10'd0, W0});                acc;
            element(0, patmode, 2, {8'd0, W1, R0});             acc;
            element(0, patmode, 2, {8'd0, W0, R1});             acc;
            element(1, patmode, 2, {8'd0, W1, R0});             acc;
            element(1, patmode, 2, {8'd0, W0, R1});             acc;
            element(0, patmode, 1, {10'd0, R0});                acc;
        end
    endtask

    task set_timing(input [7:0] smp, input [6:0] fin, input [7:0] ras_r);
        begin
            //            t_ras_r t_cas_r t_cas_f t_mux
            wr(7'h04, {ras_r, 8'd56, 8'd14, 8'd10});
            //            t_smp2 t_smp t_end t_ras_f
            wr(7'h05, {8'd64, smp, 8'd96, 8'd2});
            wr(7'h06, {16'd0, 8'd10, 1'b0, fin});
        end
    endtask

    integer s, first_pass;
    real    meas;
    initial begin
        total_err = 0;
        #100 rst = 0; #100;
        rdreg(7'h00);
        if (rd !== 32'h52414D31) begin $display("FAIL: bad ID %h", rd); $finish; end
        wr(7'h03, {24'd0, AW[3:0], AW[3:0]});
        wr(7'h07, 24'd4000);                        // refresh request every 20 us
        wr(7'h08, 0); wr(7'h09, (1 << (2 * AW)) - 1);
        wr(7'h0B, 32'h55555555); wr(7'h0C, 32'h55);
        wr(7'h0D, (1 << DW) - 1); wr(7'h0E, 0);
        wr(7'h0F, (1 << DW) - 1); wr(7'h10, 0);
        wr(7'h11, 32'h0511);                        // RAS0, CAS0, WE0, OE0
        set_timing(8'd46, 7'd0, 8'd56);                  // strobe 110 ns after RAS

        case (MODE)
        0: begin
            march_c(2'd0); march_c(2'd1); march_c(2'd2);
            rdreg(7'h13); $display("fail_addr=%0d", rd);
            rdreg(7'h14); $display("fail_bits=%h", rd);
            if ((total_err != 0) == (EXPECT_FAIL != 0))
                $display("PASS: errors=%0d expect_fail=%0d", total_err, EXPECT_FAIL);
            else
                $display("FAIL: errors=%0d expect_fail=%0d", total_err, EXPECT_FAIL);
        end
        1: begin
            // write once, then move the strobe earlier until reads fail
            element(0, 2'd2, 1, {10'd0, W0});
            first_pass = -1;
            for (s = 8; s <= 46 && first_pass < 0; s = s + 1) begin
                set_timing(s[7:0], 7'd0, 8'd56);
                element(0, 2'd2, 1, {10'd0, R0});
                rdreg(7'h12);
                if (rd == 0) first_pass = s;
            end
            // strobe tick minus RAS tick, minus the calibrated buffer delays
            meas = (first_pass - 2) * 2.5 - T_DRV - T_RX;
            $display("first passing strobe tick=%0d  measured tRAC=%0.1f ns  model=%0.1f ns",
                     first_pass, meas, T_RAC);
            if (first_pass > 0 && meas >= T_RAC - 0.01 && meas < T_RAC + 2.5 + 0.01)
                $display("PASS: speed within one tick");
            else
                $display("FAIL: speed measurement out of range");
        end
        2: begin
            // sample again 15 ns after CAS rises: EDO still drives, FPM floats
            // to the pull-ups. Read back an all-zero pattern.
            set_timing(8'd46, 7'd0, 8'd76);         // hold RAS low past CAS rising
            wr(7'h0B, 0); wr(7'h0C, 0);
            element(0, 2'd0, 1, {10'd0, W0});
            element(0, 2'd0, 1, {10'd0, R0});
            rdreg(7'h18);
            $display("sample2=%h (EDO=%0d)", rd[DW-1:0], EDO);
            if ((rd[DW-1:0] == 0) == (EDO != 0)) $display("PASS: mode detected");
            else $display("FAIL: mode detection");
        end
        3: begin
            // retention: write, stop refresh, wait, read
            element(0, 2'd0, 1, {10'd0, W0});
            wr(7'h01, 32'h10);                      // engine on, refresh off
            #(T_RET * 1.5);
            wr(7'h0A, {12'd0, 10'd0, R0, 2'b00, 2'd0, 1'b0, 3'd1});
            wr(7'h01, 32'h11);
            rdreg(7'h02);
            while (rd[0]) begin #2000; rdreg(7'h02); end
            rdreg(7'h12);
            if (rd != 0) $display("PASS: retention loss detected (%0d errors)", rd);
            else $display("FAIL: retention loss not detected");
        end
        endcase
        $finish;
    end

    initial begin
        #400_000_000; $display("FAIL: timeout"); $finish;
    end
endmodule
