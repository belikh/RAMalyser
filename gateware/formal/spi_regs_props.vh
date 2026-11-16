// Properties of spi_regs, included inside the module when FORMAL is defined.
// The core treats START and ABORT as one-clock pulses, so the write strobe
// must be one clock wide and appear once per 40-bit write frame.

    reg f_past = 1'b0;
    always @(posedge clk) f_past <= 1'b1;
    always @(*) assume (rst == !f_past);

    // SCK rising edges seen since the frame began
    reg [6:0] f_n = 7'd0;
    always @(posedge clk)
        if (rst || !sel) f_n <= 7'd0;
        else if (sck_rise) f_n <= f_n + 7'd1;

    always @(posedge clk) if (f_past && !rst && !$past(rst)) begin
        // the strobe is one clock wide
        if (reg_we) assert (!$past(reg_we));
        // it follows the 40th bit of a frame whose first bit was 1 (write).
        // The bit counter is six bits wide: a frame held past 64 bits would
        // strobe again at bit 104. The firmware always ends a frame at 40.
        if (reg_we) assert ($past(sel && sck_rise && is_write) && $past(f_n[5:0]) == 6'd39);
        // the bit counter is the number of clock edges in the frame
        assert (bitn == f_n[5:0]);
        // deselecting ends the frame: no strobe without a new 40 bits
        if (!$past(sel)) assert (bitn == 6'd0 && !is_write);
    end
`ifdef F_COVER
    always @(posedge clk) if (f_past && !rst) cover (reg_we);
`endif
