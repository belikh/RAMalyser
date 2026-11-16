// C interface to the Verilated gateware, one call per clock cycle.
#include "Vcosim_top.h"
#include "verilated.h"
#include <cstdint>

extern "C" {

void *cosim_new() {
    auto *ctx = new VerilatedContext;
    auto *top = new Vcosim_top{ctx};
    top->clk = 0;
    top->rst = 1;
    top->spi_cs_n = 1;
    top->eval();
    return top;
}

void cosim_free(void *h) {
    auto *top = static_cast<Vcosim_top *>(h);
    top->final();
    delete top;
}

// pins: bit0 sck, bit1 cs_n, bit2 mosi, bit3 dut_pg, bit4 rst.
// Returns bit0 miso, bit1 busy, bits 8..14 fine. The outputs are the
// values after the rising edge.
uint32_t cosim_cycle(void *h, const uint32_t q0[4], const uint32_t q1[4], uint32_t pins,
                     uint32_t d0[4], uint32_t d1[4], uint32_t oe[4]) {
    auto *top = static_cast<Vcosim_top *>(h);
    top->spi_sck = pins & 1;
    top->spi_cs_n = (pins >> 1) & 1;
    top->spi_mosi = (pins >> 2) & 1;
    top->dut_pg = (pins >> 3) & 1;
    top->rst = (pins >> 4) & 1;
    for (int i = 0; i < 4; i++) {
        top->io_q0[i] = q0[i];
        top->io_q1[i] = q1[i];
    }
    top->clk = 0;
    top->eval();
    top->clk = 1;
    top->eval();
    for (int i = 0; i < 4; i++) {
        d0[i] = top->io_d0[i];
        d1[i] = top->io_d1[i];
        oe[i] = top->io_oe[i];
    }
    return (top->spi_miso & 1) | ((top->busy & 1) << 1) | ((uint32_t)top->fine << 8);
}
}
