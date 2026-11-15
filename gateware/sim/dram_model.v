// Behavioural FPM / EDO DRAM with access-time, retention and fault injection.
// Simulation only. Time unit 1 ns.
`timescale 1ns/1ps

module dram_model #(
    parameter AW    = 5,          // row and column address bits
    parameter DW    = 8,
    parameter real T_RAC = 60.0,  // RAS to data valid
    parameter real T_CAC = 15.0,  // CAS to data valid
    parameter real T_OFF = 3.0,   // output turn-off
    parameter EDO   = 0,
    parameter real T_RET = 0.0,   // retention in ns, 0 = infinite
    // faults
    parameter STUCK_EN   = 0,     // one cell bit stuck
    parameter STUCK_ADDR = 0,
    parameter STUCK_BIT  = 0,
    parameter STUCK_VAL  = 0,
    parameter ADDR_STUCK_LINE = -1,  // address input line stuck low
    parameter DQ_SHORT_A = -1,       // two data outputs shorted (wired-AND)
    parameter DQ_SHORT_B = -1
) (
    input  wire          ras_n,
    input  wire          cas_n,
    input  wire          we_n,
    input  wire          oe_n,
    input  wire [AW-1:0] a,
    input  wire [DW-1:0] d,      // data in (after the adapter's drive buffers)
    output wire [DW-1:0] q,      // data out
    output wire          q_en
);
    reg [DW-1:0] mem [0:(1 << (2 * AW)) - 1];
    realtime     last_ref [0:(1 << AW) - 1];
    reg [AW-1:0] row, col, cbr_row;
    reg [DW-1:0] dout;
    reg          drive, valid;
    realtime     t_ras, t_cas;
    integer      k;

    wire [AW-1:0] a_eff = (ADDR_STUCK_LINE >= 0) ? (a & ~(1 << ADDR_STUCK_LINE)) : a;

    initial begin
        drive = 0; valid = 0; cbr_row = 0; dout = 0;
        for (k = 0; k < (1 << (2 * AW)); k = k + 1) mem[k] = {DW{1'bx}};
        for (k = 0; k < (1 << AW); k = k + 1) last_ref[k] = 0;
    end

    task refresh_row(input [AW-1:0] r);
        integer c;
        begin
            if (T_RET > 0.0 && ($realtime - last_ref[r]) > T_RET)
                for (c = 0; c < (1 << AW); c = c + 1)
                    mem[{r, c[AW-1:0]}] = ~mem[{r, c[AW-1:0]}];   // charge lost
            last_ref[r] = $realtime;
        end
    endtask

    function [DW-1:0] apply_faults(input [DW-1:0] v, input [2*AW-1:0] ad);
        reg [DW-1:0] r; reg w;
        begin
            r = v;
            if (STUCK_EN && ad == STUCK_ADDR) r[STUCK_BIT] = STUCK_VAL;
            if (DQ_SHORT_A >= 0 && DQ_SHORT_B >= 0) begin
                w = r[DQ_SHORT_A] & r[DQ_SHORT_B];
                r[DQ_SHORT_A] = w; r[DQ_SHORT_B] = w;
            end
            apply_faults = r;
        end
    endfunction

    always @(negedge ras_n) begin
        t_ras = $realtime;
        if (!cas_n) begin               // CAS-before-RAS refresh
            refresh_row(cbr_row);
            cbr_row = cbr_row + 1'b1;
        end else begin
            row = a_eff;
            refresh_row(row);
        end
    end

    realtime t_valid;
    always @(negedge cas_n) begin
        if (!ras_n) begin
            t_cas = $realtime;
            col = a_eff;
            if (!we_n) begin            // early write
                mem[{row, col}] = d;
                drive = 0;
            end else begin
                t_valid = t_ras + T_RAC;
                if (t_cas + T_CAC > t_valid) t_valid = t_cas + T_CAC;
                // before the access time has elapsed the outputs carry the
                // complement of the cell (worst case for the tester)
                drive = 1;
                dout = ~apply_faults(mem[{row, col}], {row, col});
                if (t_valid > $realtime)
                    dout <= #(t_valid - $realtime) apply_faults(mem[{row, col}], {row, col});
                else
                    dout = apply_faults(mem[{row, col}], {row, col});
            end
        end
    end

    // output turn-off: FPM when CAS rises, EDO when both RAS and CAS are high
    always @(posedge cas_n) if (!EDO || ras_n) drive <= #T_OFF 1'b0;
    always @(posedge ras_n) if (EDO && cas_n)  drive <= #T_OFF 1'b0;

    assign q_en = drive && !oe_n;
    assign q    = dout;
endmodule
