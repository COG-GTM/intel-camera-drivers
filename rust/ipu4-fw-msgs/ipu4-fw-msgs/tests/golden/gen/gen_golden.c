/*
 * Golden-file generator for the ipu4-fw-msgs parity tests.
 *
 * Includes the *unmodified* firmware ABI headers from the kernel driver and
 * emits byte-exact reference serializations of representative messages. The
 * Rust test `golden_parity.rs` builds the same logical messages and asserts its
 * safe serializer reproduces these bytes exactly.
 *
 * Build & run (done once; the .bin outputs are committed):
 *   cc -I../../../../../drivers/media/pci/intel-ipu4 \
 *      gen_golden.c -o gen_golden && ./gen_golden <out_dir>
 *
 * This file is NOT part of the kernel build and does not modify any C source.
 */

#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

/* Kernel annotations / forward declarations so the headers compile standalone. */
#define __iomem
struct device;
struct intel_ipu4_bus_device;
struct intel_ipu4_fw_com_context;

#include "intel-ipu4-isysapi-fw-types.h"
#include "intel-ipu4-fw-com.h"
#include "intel-ipu4-isys-fw-msgs.h"
#include "intel-ipu4-isys-fw-tables.h"

static void dump(const char *dir, const char *name, const void *p, size_t n)
{
	char path[1024];
	snprintf(path, sizeof(path), "%s/%s", dir, name);
	FILE *f = fopen(path, "wb");
	if (!f) {
		perror(path);
		exit(1);
	}
	if (fwrite(p, 1, n, f) != n) {
		perror("fwrite");
		exit(1);
	}
	fclose(f);
	printf("%-28s %4zu bytes\n", name, n);
}

int main(int argc, char **argv)
{
	const char *dir = argc > 1 ? argv[1] : ".";

	/* Sanity: print sizes so the Rust SIZEOF_* constants can be checked. */
	printf("sizeof resolution        = %zu\n", sizeof(struct ipu_fw_isys_resolution_abi));
	printf("sizeof output_pin_payload= %zu\n", sizeof(struct ipu_fw_isys_output_pin_payload_abi));
	printf("sizeof output_pin_info   = %zu\n", sizeof(struct ipu_fw_isys_output_pin_info_abi));
	printf("sizeof param_pin         = %zu\n", sizeof(struct ipu_fw_isys_param_pin_abi));
	printf("sizeof input_pin_info    = %zu\n", sizeof(struct ipu_fw_isys_input_pin_info_abi));
	printf("sizeof isa_cfg           = %zu\n", sizeof(struct ipu_fw_isys_isa_cfg_abi));
	printf("sizeof cropping          = %zu\n", sizeof(struct ipu_fw_isys_cropping_abi));
	printf("sizeof stream_cfg_data   = %zu\n", sizeof(struct ipu_fw_isys_stream_cfg_data_abi));
	printf("sizeof frame_buff_set    = %zu\n", sizeof(struct ipu_fw_isys_frame_buff_set_abi));
	printf("sizeof error_info        = %zu\n", sizeof(struct ipu_fw_isys_error_info_abi));
	printf("sizeof resp_info         = %zu\n", sizeof(struct ipu_fw_isys_resp_info_abi));
	printf("sizeof proxy_error_info  = %zu\n", sizeof(struct ipu_fw_isys_proxy_error_info_abi));
	printf("sizeof proxy_resp_info   = %zu\n", sizeof(struct ipu_fw_isys_proxy_resp_info_abi));
	printf("sizeof send_queue_token  = %zu\n", sizeof(struct ipu_fw_send_queue_token));
	printf("sizeof proxy_send_token  = %zu\n", sizeof(struct ipu_fw_proxy_send_queue_token));

	/* --- stream_cfg_data: 4 input pins, 6 output pins, full crop --- */
	struct ipu_fw_isys_stream_cfg_data_abi cfg;
	memset(&cfg, 0, sizeof(cfg));
	cfg.isa_cfg.isa_res[0].width = 100;
	cfg.isa_cfg.isa_res[0].height = 200;
	cfg.isa_cfg.isa_res[1].width = 300;
	cfg.isa_cfg.isa_res[1].height = 400;
	cfg.isa_cfg.cfg.blc = 1;
	cfg.isa_cfg.cfg.ae = 1;
	cfg.isa_cfg.cfg.paf = 0x5a;
	cfg.isa_cfg.cfg.send_resp_stats_ready = 1;
	for (int i = 0; i < N_IPU_FW_ISYS_CROPPING_LOCATION; i++) {
		cfg.crop[i].top_offset = i + 1;
		cfg.crop[i].left_offset = -(i + 1);
		cfg.crop[i].bottom_offset = 1000 + i;
		cfg.crop[i].right_offset = 2000 + i;
	}
	cfg.nof_input_pins = INTEL_IPU4_MAX_IPINS;
	for (int i = 0; i < INTEL_IPU4_MAX_IPINS; i++) {
		cfg.input_pins[i].input_res.width = 1920 + i;
		cfg.input_pins[i].input_res.height = 1080 + i;
		cfg.input_pins[i].dt = 0x2b; /* RAW_10 */
		cfg.input_pins[i].mipi_store_mode = 1;
		cfg.input_pins[i].bits_per_pix =
			(uint8_t)extracted_bits_per_pixel_per_mipi_data_type[0x2b];
	}
	cfg.nof_output_pins = INTEL_IPU4_MAX_OPINS;
	for (int i = 0; i < INTEL_IPU4_MAX_OPINS; i++) {
		cfg.output_pins[i].output_res.width = 1920;
		cfg.output_pins[i].output_res.height = 1080;
		cfg.output_pins[i].stride = 3840;
		cfg.output_pins[i].watermark_in_lines = i;
		cfg.output_pins[i].send_irq = 1;
		cfg.output_pins[i].input_pin_id = i % INTEL_IPU4_MAX_IPINS;
		cfg.output_pins[i].pt = 0;
		cfg.output_pins[i].ft = 20;
		cfg.output_pins[i].online = 1;
	}
	cfg.compfmt = 0xdeadbeef;
	cfg.send_irq_sof_discarded = 1;
	cfg.send_resp_eof_discarded = 1;
	cfg.src = 2;
	cfg.vc = 1;
	cfg.isl_use = IPU_FW_ISYS_USE_SINGLE_ISA;
	dump(dir, "stream_cfg_full.bin", &cfg, sizeof(cfg));

	/* --- stream_cfg_data: minimal (1 input pin, 0 output pins) --- */
	struct ipu_fw_isys_stream_cfg_data_abi mcfg;
	memset(&mcfg, 0, sizeof(mcfg));
	mcfg.nof_input_pins = 1;
	mcfg.input_pins[0].input_res.width = 640;
	mcfg.input_pins[0].input_res.height = 480;
	mcfg.input_pins[0].dt = 0x24; /* RGB_888 */
	mcfg.input_pins[0].bits_per_pix =
		(uint8_t)extracted_bits_per_pixel_per_mipi_data_type[0x24];
	mcfg.isl_use = IPU_FW_ISYS_USE_NO_ISL_NO_ISA;
	dump(dir, "stream_cfg_min.bin", &mcfg, sizeof(mcfg));

	/* --- frame_buff_set --- */
	struct ipu_fw_isys_frame_buff_set_abi fbs;
	memset(&fbs, 0, sizeof(fbs));
	for (int i = 0; i < INTEL_IPU4_MAX_OPINS; i++) {
		fbs.output_pins[i].out_buf_id = 0x1000ULL + i;
		fbs.output_pins[i].addr = 0x2000 + i;
	}
	fbs.process_group_light.param_buf_id = 0x11;
	fbs.process_group_light.addr = 0x22;
	fbs.send_irq_sof = 1;
	fbs.send_resp_eof = 1;
	dump(dir, "frame_buff_set.bin", &fbs, sizeof(fbs));

	/* --- send_queue_token (complex capture command) --- */
	struct ipu_fw_send_queue_token tok;
	memset(&tok, 0, sizeof(tok));
	tok.buf_handle = 0xabcdef0123456789ULL;
	tok.payload = 0x1234;
	tok.send_type = IPU_FW_ISYS_SEND_TYPE_STREAM_CAPTURE;
	dump(dir, "send_token.bin", &tok, sizeof(tok));

	/* --- proxy_send_queue_token --- */
	struct ipu_fw_proxy_send_queue_token ptok;
	memset(&ptok, 0, sizeof(ptok));
	ptok.request_id = 0x11111111;
	ptok.region_index = 0x22222222;
	ptok.offset = 0x33333333;
	ptok.value = 0x44444444;
	dump(dir, "proxy_send_token.bin", &ptok, sizeof(ptok));

	/* --- resp_info --- */
	struct ipu_fw_isys_resp_info_abi resp;
	memset(&resp, 0, sizeof(resp));
	resp.buf_id = 0xcafef00dULL;
	resp.pin.out_buf_id = 0x55;
	resp.pin.addr = 0x66;
	resp.process_group_light.param_buf_id = 0x77;
	resp.process_group_light.addr = 0x88;
	resp.error_info.error = IPU_FW_ISYS_ERROR_HW_CONSISTENCY;
	resp.error_info.error_details = 0x99;
	resp.timestamp[0] = 0xaaaa;
	resp.timestamp[1] = 0xbbbb;
	resp.stream_handle = 3;
	resp.type = IPU_FW_ISYS_RESP_TYPE_PIN_DATA_READY;
	resp.pin_id = 2;
	resp.acc_id = 1;
	dump(dir, "resp_info.bin", &resp, sizeof(resp));

	/* --- proxy_resp_info --- */
	struct ipu_fw_isys_proxy_resp_info_abi presp;
	memset(&presp, 0, sizeof(presp));
	presp.request_id = 0x12345678;
	presp.error_info.error = IPU_FW_PROXY_ERROR_INVALID_WRITE_OFFSET;
	presp.error_info.error_details = 0xdcba;
	dump(dir, "proxy_resp_info.bin", &presp, sizeof(presp));

	return 0;
}
