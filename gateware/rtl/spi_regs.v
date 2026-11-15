// SPI slave (mode 0, MSB first) to register bus.
// Frame: 1 command byte {write, addr[6:0]} then 32 data bits.
// On a read the register value is shifted out during the data bits.
module spi_regs (
    input  wire        clk,
    input  wire        rst,
    input  wire        sck,
    input  wire        cs_n,
    input  wire        mosi,
    output wire        miso,
    output reg         reg_we,
    output reg  [6:0]  reg_addr,
    output reg  [31:0] reg_wdata,
    input  wire [31:0] reg_rdata
);
    reg [2:0] sck_s, cs_s;
    reg [1:0] mosi_s;
    always @(posedge clk) begin
        sck_s  <= {sck_s[1:0], sck};
        cs_s   <= {cs_s[1:0], cs_n};
        mosi_s <= {mosi_s[0], mosi};
    end
    wire sck_rise = (sck_s[2:1] == 2'b01);
    wire sck_fall = (sck_s[2:1] == 2'b10);
    wire sel      = !cs_s[1];

    reg [5:0]  bitn;
    reg        is_write;
    reg [31:0] sh_in, sh_out;
    assign miso = sh_out[31];

    always @(posedge clk) begin
        reg_we <= 1'b0;
        if (rst || !sel) begin
            bitn <= 0; is_write <= 0; sh_out <= 0;
        end else begin
            if (sck_rise) begin
                sh_in <= {sh_in[30:0], mosi_s[1]};
                bitn <= bitn + 6'd1;
                if (bitn == 6'd0) is_write <= mosi_s[1];
                if (bitn == 6'd7) reg_addr <= {sh_in[5:0], mosi_s[1]};
                if (bitn == 6'd39) begin
                    reg_wdata <= {sh_in[30:0], mosi_s[1]};
                    reg_we <= is_write;
                end
            end
            if (sck_fall) begin
                if (bitn == 6'd8) sh_out <= reg_rdata;
                else sh_out <= {sh_out[30:0], 1'b0};
            end
        end
    end
endmodule
