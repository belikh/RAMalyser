// Technology-independent tester core.
//
// Time base: one "tick" is half a clk period (2.5 ns at 200 MHz). Every bus
// line leaves through a DDR output cell (d0 = first half of the clk cycle,
// d1 = second half) and DQ lines return through DDR input cells (q0 sampled
// on the rising edge, q1 on the falling edge), so every DRAM edge and the
// read strobe are placed with one-tick resolution. A per-pin input delay
// (FINE, set outside this module) interpolates between ticks.
//
// The core runs "march elements": for every address in a range, up to six
// read/write operations, each a complete RAS/CAS cycle whose edges are set by
// the TIM registers. The MCU composes complete tests (March C-, checkerboard,
// retention, speed sweeps) from elements and reads back failure data.
`include "ramspec.vh"

module tester_core #(
    parameter TICK_PS = 2500         // half a clk period, reported to the MCU
) (
    input  wire         clk,
    input  wire         rst,
    // register bus (from spi_regs)
    input  wire         reg_we,
    input  wire [6:0]   reg_addr,
    input  wire [31:0]  reg_wdata,
    output reg  [31:0]  reg_rdata,   // registered: valid 2 clk after reg_addr
    // bus lines
    output reg  [`N_IO-1:0] io_d0,   // registered
    output reg  [`N_IO-1:0] io_d1,
    output reg  [`N_IO-1:0] io_oe,
    input  wire [`N_IO-1:0] io_q0,
    input  wire [`N_IO-1:0] io_q1,
    input  wire         dut_pg,      // DUT rail on and in range
    output wire [6:0]   fine,
    output wire         busy
);
    localparam ID = 32'h52414D31;    // "RAM1"

    // ---------------------------------------------------------------- regs
    reg        r_start, r_abort;
    reg        r_rfsh_en, r_cbr, r_eng_en, r_row_fast;
    reg [3:0]  r_row_bits, r_col_bits;
    reg [7:0]  t_mux, t_cas_f, t_cas_r, t_ras_r;
    reg [7:0]  t_ras_f, t_end, t_smp, t_smp2, t_we_f;
    reg [6:0]  r_fine;
    reg [23:0] r_rfsh_period;
    reg [23:0] r_addr_start, r_addr_end;
    reg [2:0]  r_nops;
    reg        r_dir;
    reg [1:0]  r_patmode;
    reg [11:0] r_ops;                // op i: bit 2i = write, bit 2i+1 = invert
    reg [39:0] r_pat, r_mask, r_drv;
    reg [3:0]  r_ras_mask, r_cas_mask;
    reg [1:0]  r_we_mask, r_oe_mask;
    reg        r_split0, r_split1;
    reg [`N_IO-1:0] r_gpio_out, r_gpio_oe;

    reg [31:0] err_count;
    reg [23:0] fail_addr;
    reg [39:0] fail_bits, fail_data, last_smp, last_smp2;
    reg        st_busy, st_err, st_done;

    assign fine = r_fine;
    assign busy = st_busy;

    // synchronised copy of all inputs for the static (GPIO) view
    reg [`N_IO-1:0] gpio_in;
    always @(posedge clk) gpio_in <= io_q0;

    reg        we_q;
    reg [6:0]  wa_q;
    reg [31:0] wd_q;
    reg [47:0] wsel;
    always @(posedge clk) begin
        we_q <= reg_we; wa_q <= reg_addr; wd_q <= reg_wdata;
        wsel <= (reg_we && reg_addr < 7'd48) ? (48'd1 << reg_addr[5:0]) : 48'd0;
    end

    always @(posedge clk) begin
        r_start <= 1'b0;
        r_abort <= 1'b0;
        if (rst) begin
            r_rfsh_en <= 0; r_cbr <= 0; r_eng_en <= 0; r_row_fast <= 0;
            r_row_bits <= 4'd9; r_col_bits <= 4'd9;
            t_ras_f <= 8'd2; t_mux <= 8'd10; t_cas_f <= 8'd14; t_we_f <= 8'd10;
            t_smp <= 8'd40; t_smp2 <= 8'd60; t_cas_r <= 8'd48; t_ras_r <= 8'd48;
            t_end <= 8'd80; r_fine <= 0; r_rfsh_period <= 24'd3000;
            r_addr_start <= 0; r_addr_end <= 0; r_nops <= 0; r_dir <= 0;
            r_patmode <= 0; r_ops <= 0; r_pat <= 0; r_mask <= 0; r_drv <= 0;
            r_ras_mask <= 4'b0001; r_cas_mask <= 4'b0001; r_we_mask <= 2'b01;
            r_oe_mask <= 2'b01; r_split0 <= 0; r_split1 <= 0;
            r_gpio_out <= 0; r_gpio_oe <= 0;
        end else begin
            // one-hot write selects, decoded a clk earlier
            if (wsel[1]) begin
                r_start <= wd_q[0]; r_abort <= wd_q[1];
                r_rfsh_en <= wd_q[2]; r_cbr <= wd_q[3];
                r_eng_en <= wd_q[4]; r_row_fast <= wd_q[5];
            end
            if (wsel[3]) begin r_row_bits <= wd_q[3:0]; r_col_bits <= wd_q[7:4]; end
            if (wsel[4]) {t_ras_r, t_cas_r, t_cas_f, t_mux} <= wd_q;
            if (wsel[5]) {t_smp2, t_smp, t_end, t_ras_f} <= wd_q;
            if (wsel[6]) begin r_fine <= wd_q[6:0]; t_we_f <= wd_q[15:8]; end
            if (wsel[7]) r_rfsh_period <= wd_q[23:0];
            if (wsel[8]) r_addr_start <= wd_q[23:0];
            if (wsel[9]) r_addr_end <= wd_q[23:0];
            if (wsel[10]) begin
                r_nops <= wd_q[2:0]; r_dir <= wd_q[3];
                r_patmode <= wd_q[5:4]; r_ops <= wd_q[19:8];
            end
            if (wsel[11]) r_pat[31:0] <= wd_q;
            if (wsel[12]) r_pat[39:32] <= wd_q[7:0];
            if (wsel[13]) r_mask[31:0] <= wd_q;
            if (wsel[14]) r_mask[39:32] <= wd_q[7:0];
            if (wsel[15]) r_drv[31:0] <= wd_q;
            if (wsel[16]) r_drv[39:32] <= wd_q[7:0];
            if (wsel[17]) begin
                r_ras_mask <= wd_q[3:0]; r_cas_mask <= wd_q[7:4];
                r_we_mask <= wd_q[9:8]; r_oe_mask <= wd_q[11:10];
                r_split0 <= wd_q[12]; r_split1 <= wd_q[13];
            end
            if (wsel[32]) r_gpio_out[31:0] <= wd_q;
            if (wsel[33]) r_gpio_out[63:32] <= wd_q;
            if (wsel[34]) r_gpio_out[95:64] <= wd_q;
            if (wsel[35]) r_gpio_out[`N_IO-1:96] <= wd_q[`N_IO-97:0];
            if (wsel[36]) r_gpio_oe[31:0] <= wd_q;
            if (wsel[37]) r_gpio_oe[63:32] <= wd_q;
            if (wsel[38]) r_gpio_oe[95:64] <= wd_q;
            if (wsel[39]) r_gpio_oe[`N_IO-1:96] <= wd_q[`N_IO-97:0];

        end
    end

    // Read-back mux in two registered stages (16:1 then 3:1). The SPI master
    // leaves many clk cycles between sending the address and reading data.
    wire [31:0] rd_tab [0:47];
    assign rd_tab[0] = ID;
    assign rd_tab[1] = {26'd0, r_row_fast, r_eng_en, r_cbr, r_rfsh_en, 2'b00};
    assign rd_tab[2] = {28'd0, dut_pg, st_done, st_err, st_busy};
    assign rd_tab[3] = {24'd0, r_col_bits, r_row_bits};
    assign rd_tab[4] = {t_ras_r, t_cas_r, t_cas_f, t_mux};
    assign rd_tab[5] = {t_smp2, t_smp, t_end, t_ras_f};
    assign rd_tab[6] = {16'd0, t_we_f, 1'b0, r_fine};
    assign rd_tab[7] = {8'd0, r_rfsh_period};
    assign rd_tab[8] = {8'd0, r_addr_start};
    assign rd_tab[9] = {8'd0, r_addr_end};
    assign rd_tab[10] = {12'd0, r_ops, 2'b00, r_patmode, r_dir, r_nops};
    assign rd_tab[11] = r_pat[31:0];
    assign rd_tab[12] = {24'd0, r_pat[39:32]};
    assign rd_tab[13] = r_mask[31:0];
    assign rd_tab[14] = {24'd0, r_mask[39:32]};
    assign rd_tab[15] = r_drv[31:0];
    assign rd_tab[16] = {24'd0, r_drv[39:32]};
    assign rd_tab[17] = {18'd0, r_split1, r_split0, r_oe_mask, r_we_mask,
                            r_cas_mask, r_ras_mask};
    assign rd_tab[18] = err_count;
    assign rd_tab[19] = {8'd0, fail_addr};
    assign rd_tab[20] = fail_bits[31:0];
    assign rd_tab[21] = {24'd0, fail_bits[39:32]};
    assign rd_tab[22] = fail_data[31:0];
    assign rd_tab[23] = {24'd0, fail_data[39:32]};
    assign rd_tab[24] = last_smp2[31:0];
    assign rd_tab[25] = {24'd0, last_smp2[39:32]};
    assign rd_tab[26] = last_smp[31:0];
    assign rd_tab[27] = {24'd0, last_smp[39:32]};
    assign rd_tab[28] = TICK_PS;
    assign rd_tab[29] = 32'd0;
    assign rd_tab[30] = 32'd0;
    assign rd_tab[31] = 32'd0;
    assign rd_tab[32] = r_gpio_out[31:0];
    assign rd_tab[33] = r_gpio_out[63:32];
    assign rd_tab[34] = r_gpio_out[95:64];
    assign rd_tab[35] = {{(128-`N_IO){1'b0}}, r_gpio_out[`N_IO-1:96]};
    assign rd_tab[36] = r_gpio_oe[31:0];
    assign rd_tab[37] = r_gpio_oe[63:32];
    assign rd_tab[38] = r_gpio_oe[95:64];
    assign rd_tab[39] = {{(128-`N_IO){1'b0}}, r_gpio_oe[`N_IO-1:96]};
    assign rd_tab[40] = gpio_in[31:0];
    assign rd_tab[41] = gpio_in[63:32];
    assign rd_tab[42] = gpio_in[95:64];
    assign rd_tab[43] = {{(128-`N_IO){1'b0}}, gpio_in[`N_IO-1:96]};
    assign rd_tab[44] = 32'd0;
    assign rd_tab[45] = 32'd0;
    assign rd_tab[46] = 32'd0;
    assign rd_tab[47] = 32'd0;
    // stage 1: twelve 4:1 muxes; stage 2: three 4:1; stage 3: 3:1
    reg [31:0] rd_q [0:11];
    reg [31:0] rd_g0, rd_g1, rd_g2;
    integer qi;
    always @(posedge clk) begin
        for (qi = 0; qi < 12; qi = qi + 1)
            rd_q[qi] <= rd_tab[{qi[3:0], reg_addr[1:0]}];
        rd_g0 <= rd_q[{2'b00, reg_addr[3:2]}];
        rd_g1 <= rd_q[{2'b01, reg_addr[3:2]}];
        rd_g2 <= rd_q[{2'b10, reg_addr[3:2]}];
        case (reg_addr[6:4])
        3'd0:    reg_rdata <= rd_g0;
        3'd1:    reg_rdata <= rd_g1;
        3'd2:    reg_rdata <= rd_g2;
        default: reg_rdata <= 32'd0;
        endcase
    end

    // ------------------------------------------------------------ sequencer
    // One access = PREP0, PREP1 (compute address and data), CYCLE (the DRAM
    // cycle itself), CHK0, CHK1 (compare), NEXT (bookkeeping). The extra
    // states only lengthen the RAS precharge time; they keep every path short
    // enough for a 200 MHz clock.
    localparam S_IDLE = 4'd0, S_PREP0 = 4'd1, S_PREP1 = 4'd2, S_CYCLE = 4'd3,
               S_CHK0 = 4'd4, S_CHK1 = 4'd5, S_NEXT = 4'd6, S_RFSH = 4'd7,
               S_PREP2 = 4'd8, S_PREP3 = 4'd9;
    reg [3:0]  state;
    reg [7:0]  cnt;                  // clk cycles into the current DRAM cycle
    reg [23:0] addr;
    reg [2:0]  opi;
    reg [23:0] rfsh_timer;
    reg        rfsh_pending;
    reg [11:0] rfsh_row;
    reg [39:0] smp, smp2;
    reg        go_pend;              // start asked for while a refresh cycle ran
    reg        abort_pend;           // abort asked for; taken at the end of the access

    // static values derived from registers (registered once, change rarely)
    reg [11:0] cmask, rmask, lomask, himask;
    reg [3:0]  sh_amt;
    reg [7:0]  c_smp, c_smp2, last_cycle;
    reg [4:0]  lane_used;
    always @(posedge clk) begin
        cmask <= (12'd1 << r_col_bits) - 12'd1;
        rmask <= (12'd1 << r_row_bits) - 12'd1;
        sh_amt <= r_row_fast ? r_row_bits : r_col_bits;
        lomask <= r_row_fast ? rmask : cmask;
        himask <= r_row_fast ? cmask : rmask;
        // strobe alignment: +3 output registers, +1 (q0) or +2 (q1) input cell
        c_smp  <= t_smp[7:1]  + (t_smp[0]  ? 8'd5 : 8'd4);
        c_smp2 <= t_smp2[7:1] + (t_smp2[0] ? 8'd5 : 8'd4);
        // the cycle runs long enough for the last strobe to come back
        last_cycle <= {1'b0, t_end[7:1]} + 8'd6;
        lane_used <= {|r_drv[39:32], |r_drv[31:24], |r_drv[23:16],
                      |r_drv[15:8], |r_drv[7:0]};
    end

    // per-access values
    reg [11:0] lo_r, sh_r, row_r, col_r;
    reg [7:0]  ah_r;
    reg        opw_r, opinv_r, last_addr_r, last_op_r, err_r, eq_start_r, eq_end_r;
    reg [39:0] wdata_r, exp_r, diff_r;

    wire [11:0] hi_w  = sh_r & himask;
    wire [11:0] col_w = r_row_fast ? hi_w : lo_r;
    wire [11:0] row_w = r_row_fast ? lo_r : hi_w;
    reg         chk_r;
    reg  [39:0] base_r;
    reg  [39:0] base;
    always @(*) begin
        case (r_patmode)
        2'd0:    base = r_pat;
        2'd1:    base = r_pat ^ {40{chk_r}};
        default: base = r_pat ^ {5{ah_r}};
        endcase
    end
    wire [39:0] wdata_w = opinv_r ? ~base_r : base_r;

    // registered strobe hits and end-of-cycle flag (one clk of look-ahead)
    reg        cycle_done, hit1, hit2, go_r, sat_r;
    reg [39:0] q0_r, q1_r;
    always @(posedge clk) begin
        cycle_done <= (cnt == last_cycle - 8'd1) && (state == S_CYCLE || state == S_RFSH);
        hit1 <= (cnt == c_smp)  && (state == S_CYCLE);
        hit2 <= (cnt == c_smp2) && (state == S_CYCLE);
        q0_r <= io_q0[39:0]; q1_r <= io_q1[39:0];
        go_r  <= r_start && r_nops != 0 && r_eng_en;
        sat_r <= &err_count;
        if (hit1) smp  <= t_smp[0]  ? q1_r : q0_r;
        if (hit2) smp2 <= t_smp2[0] ? q1_r : q0_r;
    end

    always @(posedge clk) begin
        if (rst) begin
            state <= S_IDLE; cnt <= 0; addr <= 0; opi <= 0;
            rfsh_timer <= 0; rfsh_pending <= 0; rfsh_row <= 0;
            st_busy <= 0; st_err <= 0; st_done <= 0; go_pend <= 0; abort_pend <= 0;
            err_count <= 0; fail_addr <= 0; fail_bits <= 0; fail_data <= 0;
            last_smp <= 0; last_smp2 <= 0;
            opw_r <= 0; opinv_r <= 0; last_addr_r <= 0; last_op_r <= 0; err_r <= 0;
        end else begin
            // refresh request timer (counts down, reloads on zero)
            if (!r_rfsh_en) begin
                rfsh_timer <= r_rfsh_period; rfsh_pending <= 0;
            end else if (rfsh_timer == 24'd0) begin
                rfsh_timer <= r_rfsh_period; rfsh_pending <= 1'b1;
            end else
                rfsh_timer <= rfsh_timer - 24'd1;

            // A start is a one-clk pulse. If it arrives during an idle
            // refresh cycle it is held until the cycle ends, and BUSY rises
            // at once so the MCU never reads the previous element's results.
            if (go_r && !st_busy) begin
                go_pend <= 1; st_busy <= 1; st_done <= 0;
            end

            // An abort is a one-clk pulse too, and the sequencer only looks
            // for it in one state of each access: hold it until then.
            if (r_abort && st_busy) abort_pend <= 1;

            case (state)
            S_IDLE: begin
                cnt <= 0;
                if (go_r || go_pend) begin
                    go_pend <= 0;
                    st_busy <= 1; st_done <= 0; st_err <= 0;
                    err_count <= 0; fail_bits <= 0;
                    addr <= r_dir ? r_addr_end : r_addr_start;
                    opi <= 0;
                    state <= S_PREP0;
                end else if (rfsh_pending && r_eng_en) begin
                    rfsh_pending <= 0;
                    state <= S_RFSH;
                end
            end
            S_PREP0: begin
                sh_r <= addr >> sh_amt;
                lo_r <= addr[11:0] & lomask;
                ah_r <= addr[7:0] ^ addr[15:8] ^ addr[23:16];
                opw_r <= r_ops[{opi, 1'b0}];
                opinv_r <= r_ops[{opi, 1'b1}];
                eq_start_r <= (addr == r_addr_start);
                eq_end_r <= (addr == r_addr_end);
                last_op_r <= (opi + 3'd1 == r_nops);
                state <= S_PREP1;
            end
            S_PREP1: begin
                row_r <= row_w; col_r <= col_w;
                chk_r <= row_w[0] ^ col_w[0];
                last_addr_r <= r_dir ? eq_start_r : eq_end_r;
                state <= S_PREP2;
            end
            S_PREP2: begin
                base_r <= base;
                state <= S_PREP3;
            end
            S_PREP3: begin
                wdata_r <= wdata_w;
                exp_r <= wdata_w;
                if (r_split0) exp_r[1]  <= wdata_w[0];
                if (r_split1) exp_r[33] <= wdata_w[32];
                cnt <= 0;
                state <= S_CYCLE;
            end
            S_CYCLE: begin
                if (!cycle_done) cnt <= cnt + 8'd1;
                else begin cnt <= 0; state <= S_CHK0; end
            end
            S_CHK0: begin
                diff_r <= (smp ^ exp_r) & r_mask;
                state <= S_CHK1;
            end
            S_CHK1: begin
                err_r <= !opw_r && (|diff_r);
                state <= S_NEXT;
            end
            S_NEXT: begin
                if (!opw_r) begin last_smp <= smp; last_smp2 <= smp2; end
                if (err_r) begin
                    if (!st_err) begin fail_addr <= addr; fail_data <= smp; end
                    st_err <= 1;
                    fail_bits <= fail_bits | diff_r;
                    if (!sat_r) err_count <= err_count + 32'd1;
                end
                if (r_abort || abort_pend || (last_op_r && last_addr_r)) begin
                    abort_pend <= 0;
                    st_busy <= 0; st_done <= 1; state <= S_IDLE;
                end else begin
                    if (!last_op_r) opi <= opi + 3'd1;
                    else begin
                        opi <= 0;
                        addr <= r_dir ? addr - 24'd1 : addr + 24'd1;
                    end
                    if (rfsh_pending) begin rfsh_pending <= 0; state <= S_RFSH; end
                    else state <= S_PREP0;
                end
            end
            S_RFSH: begin
                if (!cycle_done) cnt <= cnt + 8'd1;
                else begin
                    cnt <= 0;
                    rfsh_row <= rfsh_row + 12'd1;
                    // back into the element it interrupted, or to idle
                    // (where a held start is taken)
                    state <= (st_busy && !go_pend) ? S_PREP0 : S_IDLE;
                end
            end
            default: state <= S_IDLE;
            endcase
        end
    end

    // ---------------------------------------------------------- wave shaping
    // A signal is low for ticks [from, to). Tick 2*cnt is the first half of
    // clk cycle cnt and 2*cnt+1 the second, so "tick >= T" reduces to a
    // compare of cnt against a threshold that only depends on T:
    //   first half:  cnt >= T[7:1] + T[0]      second half: cnt >= T[7:1]
    // Thresholds are registered once; stage A registers the compare results,
    // stage B combines them into levels, and a final register drives the
    // DDR cells. Outputs therefore lag cnt by three clk cycles.
    reg [7:0] th_ras_f, th_ras_r, th_cas_f, th_cas_r, th_we_f, th_mux, th_end,
              th_rasc, th_endm2;
    reg       h_ras_f, h_ras_r, h_cas_f, h_cas_r, h_we_f, h_mux, h_end,
              h_rasc, h_endm2;
    wire [7:0] t_rasc  = t_ras_f + 8'd4;      // CBR refresh: RAS after CAS
    wire [7:0] t_endm2 = t_end - 8'd2;
    always @(posedge clk) begin
        {th_ras_f, h_ras_f} <= {1'b0, t_ras_f};  {th_ras_r, h_ras_r} <= {1'b0, t_ras_r};
        {th_cas_f, h_cas_f} <= {1'b0, t_cas_f};  {th_cas_r, h_cas_r} <= {1'b0, t_cas_r};
        {th_we_f,  h_we_f}  <= {1'b0, t_we_f};   {th_mux,   h_mux}   <= {1'b0, t_mux};
        {th_end,   h_end}   <= {1'b0, t_end};    {th_rasc,  h_rasc}  <= {1'b0, t_rasc};
        {th_endm2, h_endm2} <= {1'b0, t_endm2};
    end

    // stage A: ge1_x = second-half compare, ge0_x = first-half compare
    reg ge0_ras_f, ge1_ras_f, ge0_ras_r, ge1_ras_r, ge0_cas_f, ge1_cas_f,
        ge0_cas_r, ge1_cas_r, ge0_we_f, ge1_we_f, ge0_mux, ge1_mux,
        ge0_end, ge1_end, ge0_rasc, ge1_rasc, ge0_endm2, ge1_endm2, ge_one;
    reg cyc_a, rf_a;
    always @(posedge clk) begin
        ge1_ras_f <= cnt >= th_ras_f;  ge0_ras_f <= cnt >= th_ras_f + h_ras_f;
        ge1_ras_r <= cnt >= th_ras_r;  ge0_ras_r <= cnt >= th_ras_r + h_ras_r;
        ge1_cas_f <= cnt >= th_cas_f;  ge0_cas_f <= cnt >= th_cas_f + h_cas_f;
        ge1_cas_r <= cnt >= th_cas_r;  ge0_cas_r <= cnt >= th_cas_r + h_cas_r;
        ge1_we_f  <= cnt >= th_we_f;   ge0_we_f  <= cnt >= th_we_f + h_we_f;
        ge1_mux   <= cnt >= th_mux;    ge0_mux   <= cnt >= th_mux + h_mux;
        ge1_end   <= cnt >= th_end;    ge0_end   <= cnt >= th_end + h_end;
        ge1_rasc  <= cnt >= th_rasc;   ge0_rasc  <= cnt >= th_rasc + h_rasc;
        ge1_endm2 <= cnt >= th_endm2;  ge0_endm2 <= cnt >= th_endm2 + h_endm2;
        ge_one    <= cnt >= 8'd1;      // tick >= 2
        cyc_a <= (state == S_CYCLE);
        rf_a  <= (state == S_RFSH);
    end

    // stage B: levels (1 = asserted, i.e. the pin is driven low)
    reg ras0, ras1, cas0, cas1, we0, we1, oe0, oe1, drv0, drv1, wr_cyc_r;
    reg [11:0] ma0, ma1;
    wire wr_a = cyc_a && opw_r;
    wire rd_a = cyc_a && !opw_r;
    always @(posedge clk) begin
        ras0 <= (cyc_a || (rf_a && !r_cbr)) ? (ge0_ras_f && !ge0_ras_r)
              : (rf_a && r_cbr)             ? (ge0_rasc && !ge0_ras_r) : 1'b0;
        ras1 <= (cyc_a || (rf_a && !r_cbr)) ? (ge1_ras_f && !ge1_ras_r)
              : (rf_a && r_cbr)             ? (ge1_rasc && !ge1_ras_r) : 1'b0;
        cas0 <= cyc_a ? (ge0_cas_f && !ge0_cas_r) : (rf_a && r_cbr) ? !ge0_cas_r : 1'b0;
        cas1 <= cyc_a ? (ge1_cas_f && !ge1_cas_r) : (rf_a && r_cbr) ? !ge1_cas_r : 1'b0;
        we0  <= wr_a && ge0_we_f && !ge0_cas_r;
        we1  <= wr_a && ge1_we_f && !ge1_cas_r;
        oe0  <= rd_a && ge0_mux && !ge0_end;
        oe1  <= rd_a && ge1_mux && !ge1_end;
        drv0 <= wr_a && ge_one && !ge0_endm2;
        drv1 <= wr_a && ge_one && !ge1_endm2;
        wr_cyc_r <= wr_a;
        ma0 <= rf_a ? rfsh_row : (cyc_a && ge0_mux) ? col_r : row_r;
        ma1 <= rf_a ? rfsh_row : (cyc_a && ge1_mux) ? col_r : row_r;
    end

    integer i;
    reg [`N_IO-1:0] n_d0, n_d1, n_oe;
    always @(*) begin
        n_d0 = r_gpio_out; n_d1 = r_gpio_out; n_oe = r_gpio_oe;
        if (r_eng_en && dut_pg) begin
            n_d0[39:0] = wdata_r; n_d1[39:0] = wdata_r;
            n_oe[39:0] = wr_cyc_r ? r_drv : 40'd0;
            n_d0[`IO_MA0 +: 12] = ma0; n_d1[`IO_MA0 +: 12] = ma1;
            for (i = 0; i < 4; i = i + 1) begin
                n_d0[`IO_RAS0_N + i] = !(ras0 && r_ras_mask[i]);
                n_d1[`IO_RAS0_N + i] = !(ras1 && r_ras_mask[i]);
                n_d0[`IO_CAS0_N + i] = !(cas0 && r_cas_mask[i]);
                n_d1[`IO_CAS0_N + i] = !(cas1 && r_cas_mask[i]);
            end
            for (i = 0; i < 2; i = i + 1) begin
                n_d0[`IO_WE0_N + i] = !(we0 && r_we_mask[i]);
                n_d1[`IO_WE0_N + i] = !(we1 && r_we_mask[i]);
                n_d0[`IO_OE0_N + i] = !(oe0 && r_oe_mask[i]);
                n_d1[`IO_OE0_N + i] = !(oe1 && r_oe_mask[i]);
            end
            for (i = 0; i < 5; i = i + 1) begin
                n_d0[`IO_DQ_DRV_EN0_N + i] = !(drv0 && lane_used[i]);
                n_d1[`IO_DQ_DRV_EN0_N + i] = !(drv1 && lane_used[i]);
            end
            n_d0[`IO_DQ_RX_EN_N] = wr_cyc_r; n_d1[`IO_DQ_RX_EN_N] = wr_cyc_r;
            n_d0[`IO_CTL_DRV_EN_N] = 1'b0; n_d1[`IO_CTL_DRV_EN_N] = 1'b0;
            n_oe[`IO_CTL_DRV_EN_N : `IO_MA0] = {(`IO_CTL_DRV_EN_N - `IO_MA0 + 1){1'b1}};
        end else if (!dut_pg) begin
            // never back-feed an unpowered DUT or adapter
            n_oe = {`N_IO{1'b0}};
        end
    end
    always @(posedge clk) begin io_d0 <= n_d0; io_d1 <= n_d1; io_oe <= n_oe; end

`ifdef FORMAL
`include "tester_core_props.vh"
`endif
endmodule
