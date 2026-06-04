# Regenerates the golden reference binaries from the *unmodified* kernel
# firmware ABI headers. Run `make` here to rebuild ../*.bin.
#
# The generator #includes the real driver headers and dumps raw struct bytes,
# so the committed .bin files are byte-exact C-compiler output for the target
# ABI (LP64, little-endian).

IPU4_DIR := ../../../../../../drivers/media/pci/intel-ipu4
CFLAGS   := -O2 -Wall -I$(IPU4_DIR)
OUT_DIR  := ..

.PHONY: all clean
all: gen_golden
	./gen_golden $(OUT_DIR)

gen_golden: gen_golden.c
	$(CC) $(CFLAGS) $< -o $@

clean:
	rm -f gen_golden
