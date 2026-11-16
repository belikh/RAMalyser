// Properties of tester_core, included inside the module when FORMAL is
// defined (gateware/formal/run.sh). They are statements about the sequencer
// and the registered bus outputs; nothing here is synthesised.
//
// F_COVER adds reachability checks, so that the properties are known not to
// hold only because the situations they describe never occur.

    reg f_past = 1'b0;
    always @(posedge clk) f_past <= 1'b1;
    always @(*) assume (rst == !f_past);          // one clock of reset, then none

    wire f_accept = go_r && !st_busy;             // a start request the engine must take
    wire f_active = (state == S_CYCLE) || (state == S_RFSH);

    // ghosts
    reg       f_idle_rf = 1'b0;   // the refresh cycle now running was entered from idle
    reg       f_fresh = 1'b0;     // a request was accepted and no DRAM cycle has run for it yet
    reg [2:0] f_act = 3'b0;       // a cycle (access or refresh) was running 1, 2, 3 clocks ago
    reg [2:0] f_cyc = 3'b0;       // the same for an access cycle only
    reg [2:0] f_ctl = 3'b0;       // engine on and DUT power good, 1, 2, 3 clocks ago
    reg [2:0] f_warm = 3'b0;      // clocks since reset, up to 4: the output stages are not
                                  // reset and take three clocks to follow the sequencer
    always @(posedge clk) begin
        if (rst) begin
            f_idle_rf <= 0; f_fresh <= 0; f_act <= 0; f_cyc <= 0; f_ctl <= 0; f_warm <= 0;
        end else begin
            if (state == S_IDLE) f_idle_rf <= 1'b1;
            else if (state != S_RFSH) f_idle_rf <= 1'b0;
            if (f_accept) f_fresh <= 1'b1;
            else if (state == S_CYCLE) f_fresh <= 1'b0;
            f_act <= {f_act[1:0], f_active};
            f_cyc <= {f_cyc[1:0], state == S_CYCLE};
            f_ctl <= {f_ctl[1:0], r_eng_en && dut_pg};
            if (f_warm != 3'd4) f_warm <= f_warm + 3'd1;
        end
    end

    integer fi;
    always @(posedge clk) if (f_past && !rst && !$past(rst)) begin
        // ---- the state register only holds defined states
        assert (state <= S_PREP3);

        // ---- a start request is never lost
        // BUSY is up and DONE is down on the clock after the request, in
        // whatever state the request found the sequencer
        if ($past(f_accept)) assert (st_busy && !st_done);
        // a held request is only ever waiting for idle
        if (go_pend) assert (st_busy && !st_done && (state == S_IDLE || state == S_RFSH));
        // idle with BUSY up means a request is held, and it is taken now
        if (state == S_IDLE && st_busy) assert (go_pend);
        if ($past(state == S_IDLE) && $past(go_pend || go_r))
            assert (state == S_PREP0 && st_busy && !st_done && !st_err && err_count == 0
                    && !go_pend);
        // a refresh entered from idle with BUSY up is holding a request;
        // a refresh inside an element is not
        if (state == S_RFSH && st_busy) assert (go_pend == f_idle_rf);
        if (state == S_RFSH && !st_busy) assert (f_idle_rf);
        // outside idle and idle refresh the engine is busy
        if (!st_busy) assert (state == S_IDLE || state == S_RFSH);

        // ---- BUSY and DONE as the firmware reads them
        assert (!(st_busy && st_done));
        if ($rose(st_done)) assert ($past(state) == S_NEXT && state == S_IDLE);
        if ($fell(st_busy)) assert (st_done);
        // DONE is never up for a request whose first DRAM cycle has not run
        assert (!(st_done && f_fresh));

        // ---- abort
        // an abort request is held unless the element ends on that very clock
        if ($past(r_abort && st_busy) && $past(state) != S_NEXT) assert (abort_pend);
        if (abort_pend) assert (st_busy);
        if ($past(state == S_NEXT) && $past(abort_pend || r_abort))
            assert (!st_busy && st_done && state == S_IDLE && !abort_pend);

        // ---- the pins
        // DUT power not good: every line released
        if (!$past(dut_pg)) assert (io_oe == {`N_IO{1'b0}});
    end

    always @(posedge clk) if (f_past && !rst && f_warm == 3'd4) begin
        // ---- strobes at the stage that feeds the pins
        // a write (WE, FPGA data drivers) and a read (OE) never overlap
        assert (!((we0 || we1 || drv0 || drv1 || wr_cyc_r) && (oe0 || oe1)));
        // CAS-before-RAS refresh is the only way CAS is low outside an access
        if ((cas0 || cas1) && !f_cyc[1]) assert ($past(r_cbr) && f_act[1]);
        // RAS is only ever low inside an access or a refresh
        if (ras0 || ras1) assert (f_act[1]);

        if (f_ctl[0]) begin
            // the FPGA drives DQ only with the adapter's receivers off and OE high
            if (io_oe[39:0] != 40'd0)
                assert (io_d0[`IO_DQ_RX_EN_N] && io_d1[`IO_DQ_RX_EN_N]
                        && io_d0[`IO_OE0_N +: 2] == 2'b11 && io_d1[`IO_OE0_N +: 2] == 2'b11);
            // no cycle running: every strobe high, DQ released, no driver enabled
            if (!f_act[2]) begin
                assert (io_d0[`IO_RAS0_N +: 4] == 4'hF && io_d1[`IO_RAS0_N +: 4] == 4'hF);
                assert (io_d0[`IO_CAS0_N +: 4] == 4'hF && io_d1[`IO_CAS0_N +: 4] == 4'hF);
            end
            // no access running (idle or refresh): no WE, no OE, DQ released
            if (!f_cyc[2]) begin
                assert (io_d0[`IO_WE0_N +: 2] == 2'b11 && io_d1[`IO_WE0_N +: 2] == 2'b11);
                assert (io_d0[`IO_OE0_N +: 2] == 2'b11 && io_d1[`IO_OE0_N +: 2] == 2'b11);
                assert (io_d0[`IO_DQ_DRV_EN0_N +: 5] == 5'h1F
                        && io_d1[`IO_DQ_DRV_EN0_N +: 5] == 5'h1F);
                assert (io_oe[39:0] == 40'd0);
            end
        end
    end

    // ---- progress: no state can hold the sequencer for ever
    // The cycle counter runs from zero to the cycle length and the cycle
    // then ends, so an access or a refresh lasts at most last_cycle + 1
    // clocks (134 at the longest setting). Every other state except idle
    // lasts one clock. The one assumption: the cycle length is not rewritten
    // while a cycle runs.
    always @(posedge clk) if (f_past && f_active) assume ($stable(last_cycle));
    always @(posedge clk) if (f_past && !rst && !$past(rst)) begin
        if (!f_active) assert (cnt == 8'd0);
        if (f_active) begin
            assert (cnt <= last_cycle);
            assert (cycle_done == (cnt == last_cycle));
        end
        if (state != S_IDLE && !f_active) assert (state != $past(state));
        // an access always moves forward: PREP0..PREP3, cycle, CHK0, CHK1, NEXT
        case ($past(state))
        S_PREP0: assert (state == S_PREP1);
        S_PREP1: assert (state == S_PREP2);
        S_PREP2: assert (state == S_PREP3);
        S_PREP3: assert (state == S_CYCLE);
        S_CYCLE: assert (state == S_CYCLE || state == S_CHK0);
        S_CHK0:  assert (state == S_CHK1);
        S_CHK1:  assert (state == S_NEXT);
        S_NEXT:  assert (state == S_IDLE || state == S_PREP0 || state == S_RFSH);
        S_RFSH:  assert (state == S_RFSH || state == S_IDLE || state == S_PREP0);
        S_IDLE:  assert (state == S_IDLE || state == S_PREP0 || state == S_RFSH);
        endcase
        // a start held through a refresh cycle is still held when the cycle
        // ends in idle, where it is taken on the next clock
        if ($past(state) == S_RFSH && state == S_IDLE && $past(go_pend || f_accept))
            assert (go_pend);
        // an abort is held until the access that is running ends, and that
        // access is the last: with the steps above, at most one refresh and
        // one access after the request
        if ($past(abort_pend) && !abort_pend) assert ($past(state) == S_NEXT);
        if ($past(abort_pend) && $past(state) == S_NEXT) assert (state == S_IDLE);
    end

`ifdef F_COVER
    // reachability: the interesting situations exist, so the checks are not vacuous
    always @(posedge clk) if (f_past && !rst) begin
        cover (go_pend && state == S_RFSH);            // start held during idle refresh
        cover ($past(go_pend) && state == S_PREP0);    // and then taken
        cover (st_done && !st_err);                    // an element finished
        cover (st_done && st_err);                     // an element finished with errors
        cover (state == S_RFSH && st_busy && !go_pend);   // refresh inside an element
        cover ($past(abort_pend) && st_done);          // an abort ended an element
    end
`endif
