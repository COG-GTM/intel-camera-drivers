//! Shared test fixtures: an independent CPD binary builder.
//!
//! This builder is deliberately written from scratch (not via the `ipu4-cpd`
//! crate) so it can act as an *independent reference* for the golden-file parity
//! test: it encodes the same on-disk layout the C structs describe, and the
//! parser's output is checked field-by-field against the values poked in here.

#![allow(dead_code)]

/// `$CPD` marker.
pub const CPD_HDR_MARK: u32 = 0x4450_4324;
/// Firmware-package release stamped into the moduledata header.
pub const FW_PKG_RELEASE: u32 = 0xCAFE_1234;
/// Metadata extension type (IUNIT).
pub const EXTN_TYPE_IUNIT: u32 = 0x10;
/// Metadata image type (main firmware).
pub const IMG_TYPE_MAIN_FIRMWARE: u32 = 2;

const SIZEOF_CPD_HDR: usize = 16;
const SIZEOF_CPD_ENT: usize = 24;
const SIZEOF_METADATA_EXTN: usize = 28;
const SIZEOF_METADATA_CMPNT: usize = 68;
const SIZEOF_MODULE_DATA_HDR: usize = 44;

fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// Builder for syntactically/semantically valid CPD files, with knobs to
/// produce specific malformed variants for negative tests.
#[derive(Debug, Clone)]
pub struct CpdBuilder {
    components: usize,
    manifest_len: usize,
    payload_size: usize,
    hdr_mark: u32,
    fw_pkg_date: u32,
    extn_type: u32,
    img_type: u32,
}

impl Default for CpdBuilder {
    fn default() -> Self {
        Self {
            components: 2,
            manifest_len: 64,
            payload_size: 32,
            hdr_mark: CPD_HDR_MARK,
            fw_pkg_date: FW_PKG_RELEASE,
            extn_type: EXTN_TYPE_IUNIT,
            img_type: IMG_TYPE_MAIN_FIRMWARE,
        }
    }
}

impl CpdBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn components(mut self, n: usize) -> Self {
        self.components = n;
        self
    }
    pub fn manifest_len(mut self, n: usize) -> Self {
        self.manifest_len = n;
        self
    }
    pub fn payload_size(mut self, n: usize) -> Self {
        self.payload_size = n;
        self
    }
    pub fn hdr_mark(mut self, v: u32) -> Self {
        self.hdr_mark = v;
        self
    }
    pub fn fw_pkg_date(mut self, v: u32) -> Self {
        self.fw_pkg_date = v;
        self
    }
    pub fn extn_type(mut self, v: u32) -> Self {
        self.extn_type = v;
        self
    }
    pub fn img_type(mut self, v: u32) -> Self {
        self.img_type = v;
        self
    }

    // ---- expected component values (the parity oracle) -------------------

    pub fn component_id(&self, i: usize) -> u32 {
        i as u32
    }
    pub fn component_size(&self) -> u32 {
        self.payload_size as u32
    }
    pub fn component_ver(&self, i: usize) -> u32 {
        0x100 + i as u32
    }
    pub fn component_entry_point(&self, i: usize) -> u32 {
        0xE000 + i as u32
    }
    pub fn component_icache(&self, i: usize) -> u32 {
        0xC000 + i as u32
    }
    pub fn component_count(&self) -> usize {
        self.components
    }

    /// Build the metadata section (extension header + component table).
    pub fn build_metadata(&self) -> Vec<u8> {
        let mut m = vec![0u8; SIZEOF_METADATA_EXTN + self.components * SIZEOF_METADATA_CMPNT];
        put_u32(&mut m, 0, self.extn_type);
        put_u32(&mut m, 4, 0); // extn.len (unused by validator)
        put_u32(&mut m, 8, self.img_type);
        for i in 0..self.components {
            let c = SIZEOF_METADATA_EXTN + i * SIZEOF_METADATA_CMPNT;
            put_u32(&mut m, c, self.component_id(i));
            put_u32(&mut m, c + 4, self.component_size());
            put_u32(&mut m, c + 8, self.component_ver(i));
            // sha2_hash[32] at c+12 left zero
            put_u32(&mut m, c + 44, self.component_entry_point(i));
            put_u32(&mut m, c + 48, self.component_icache(i));
            // attrs[16] at c+52 left zero
        }
        m
    }

    /// Build the moduledata section (header + embedded directory + payloads).
    pub fn build_moduledata(&self) -> Vec<u8> {
        let n = self.components;
        let dir_hdr_off = SIZEOF_MODULE_DATA_HDR; // 44
        let dir_ent_off = dir_hdr_off + SIZEOF_CPD_HDR; // 60
        let payloads_off = dir_ent_off + n * SIZEOF_CPD_ENT;
        let total = payloads_off + n * self.payload_size;
        let mut d = vec![0u8; total];

        // module_data_hdr
        put_u32(&mut d, 0, SIZEOF_MODULE_DATA_HDR as u32); // hdr_len
        put_u32(&mut d, 8, self.fw_pkg_date); // fw_pkg_date

        // embedded cpd directory header
        put_u32(&mut d, dir_hdr_off, self.hdr_mark);
        put_u32(&mut d, dir_hdr_off + 4, n as u32); // ent_cnt
        d[dir_hdr_off + 10] = SIZEOF_CPD_HDR as u8; // hdr_len

        // directory entries point at the payloads
        for i in 0..n {
            let e = dir_ent_off + i * SIZEOF_CPD_ENT;
            let off = (payloads_off + i * self.payload_size) as u32;
            put_u32(&mut d, e + 12, off);
            put_u32(&mut d, e + 16, self.payload_size as u32);
        }
        d
    }

    /// Offset (within moduledata) of directory entry `i`'s payload.
    pub fn moduledata_payload_offset(&self, i: usize) -> u32 {
        let dir_ent_off = SIZEOF_MODULE_DATA_HDR + SIZEOF_CPD_HDR;
        let payloads_off = dir_ent_off + self.components * SIZEOF_CPD_ENT;
        (payloads_off + i * self.payload_size) as u32
    }

    /// Build a complete CPD file.
    pub fn build(&self) -> Vec<u8> {
        let manifest = vec![0xABu8; self.manifest_len];
        let metadata = self.build_metadata();
        let moduledata = self.build_moduledata();

        let man_off = SIZEOF_CPD_HDR + 3 * SIZEOF_CPD_ENT; // 88
        let met_off = man_off + manifest.len();
        let mod_off = met_off + metadata.len();

        let mut f = vec![0u8; man_off];
        put_u32(&mut f, 0, self.hdr_mark);
        put_u32(&mut f, 4, 3); // ent_cnt
        f[10] = SIZEOF_CPD_HDR as u8; // hdr_len

        let set_entry = |f: &mut [u8], idx: usize, off: usize, len: usize| {
            let e = SIZEOF_CPD_HDR + idx * SIZEOF_CPD_ENT;
            put_u32(f, e + 12, off as u32);
            put_u32(f, e + 16, len as u32);
        };
        set_entry(&mut f, 0, man_off, manifest.len());
        set_entry(&mut f, 1, met_off, metadata.len());
        set_entry(&mut f, 2, mod_off, moduledata.len());

        f.extend_from_slice(&manifest);
        f.extend_from_slice(&metadata);
        f.extend_from_slice(&moduledata);
        f
    }
}
