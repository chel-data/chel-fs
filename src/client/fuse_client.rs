/*
 *  Copyright (C) 2024 github.com/chel-data
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.

 */

use crate::bindings::daos::{
    DAOS_COND_DKEY_FETCH,
    DAOS_COND_DKEY_UPDATE,
    DAOS_OO_RO,
    DAOS_REC_ANY,
    DAOS_TXN_NONE,
    DER_NONEXIST,
    daos_anchor_is_eof,
    daos_anchor_t,
    daos_cont_props_DAOS_PROP_CO_ROOTS,
    daos_cont_query,
    daos_handle_t,
    daos_iod_t,
    daos_iod_type_t_DAOS_IOD_SINGLE,
    daos_key_desc_t,
    daos_key_t,
    daos_obj_close,
    daos_obj_fetch,
    daos_obj_id_t,
    daos_obj_list_dkey,
    daos_obj_open,
    daos_obj_update,
    daos_prop_alloc,
    daos_prop_co_roots,
    daos_prop_entry_get,
    daos_prop_free,
    daos_prop_t,
    d_iov_t,
    d_sg_list_t,
};
use crate::utils::daos_conn::DAOSConn;
use fuser::{
    consts::{FOPEN_DIRECT_IO, FOPEN_NONSEEKABLE},
    FileAttr,
    FileType,
    Filesystem,
    MountOption,
    ReplyAttr,
    ReplyData,
    ReplyCreate,
    ReplyDirectory,
    ReplyEmpty,
    ReplyEntry,
    ReplyOpen,
    Request,
};
use libc::{EFAULT, EINVAL, ENOENT, ENOMEM, EOPNOTSUPP};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::CString,
    ffi::OsStr,
    ops::Add,
    option::Option,
    os::raw::c_void,
    os::unix::ffi::OsStrExt,
    path::Path,
    ptr,
    result::Result,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
    vec::Vec,
};
use zvariant::{LE, Type, serialized::Context, serialized::Data, to_bytes};

const ROOT_INODE_NUMBER: u64 = 0;
const INODE_AKEY: &str = "INODE_ENTRY";

struct RawWrapper<T> {
    raw_ptr: *mut T,
    deallocator: fn(*mut T) -> (),
}

impl<T> Drop for RawWrapper<T> {
    fn drop(&mut self) {
        (self.deallocator)(self.raw_ptr);
    }
}

struct RawTWrapper<T> where T: Copy {
    obj: T,
    deallocator: fn(T) -> (),
}

impl<T: Copy> Drop for RawTWrapper<T> {
    fn drop(&mut self) {
        (self.deallocator)(self.obj);
    }
}

#[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
struct InodeEntry {
    mode: u32,
    oid_lo: u64,
    oid_hi: u64,
    atime: u64,
    mtime: u64,
    ctime: u64,
    crtime: u64,
    uid: u32,
    gid: u32,
    inum: u64,
    chunk_size: u64,
}

impl From<InodeEntry> for FileAttr {
    fn from(attrs: InodeEntry) -> Self {
        FileAttr {
            ino: attrs.inum,
            size: attrs.chunk_size,
            blocks: 0,
            atime: UNIX_EPOCH.add(Duration::from_secs(attrs.atime)),
            mtime: UNIX_EPOCH.add(Duration::from_secs(attrs.mtime)),
            ctime: UNIX_EPOCH.add(Duration::from_secs(attrs.ctime)),
            crtime: UNIX_EPOCH.add(Duration::from_secs(attrs.crtime)),
            kind: FileType::RegularFile,
            perm: 0o444,
            nlink: 1,
            uid: attrs.uid,
            gid: attrs.gid,
            rdev: 0,
            flags: 0,
            blksize: 0,
        }
    }
}

#[derive(Debug)]
struct InodeInfo {
    oid: daos_obj_id_t,
    parent_oid: daos_obj_id_t,
    name: Vec<u8>,
}

pub struct FUSEClient {
    daos_conn: Box<DAOSConn>,
    ino_obj_map: HashMap<u64, InodeInfo>,
}

fn alloc_inum() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(3);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn encode_inode(inode: &InodeEntry) -> zvariant::Result<Data<'static, 'static>> {
    let ctxt = Context::new_dbus(LE, 0);
    to_bytes(ctxt, inode)
}

fn decode_inode(bytes: &[u8]) -> zvariant::Result<(InodeEntry, usize)> {
    let ctxt = Context::new_dbus(LE, 0);
    let raw_data = Data::new(bytes, ctxt);
    raw_data.deserialize()
}

impl FUSEClient {
    fn free_hdl(hdl: daos_handle_t) {
        let _ = FUSEClient::close_obj(hdl);
    }

    fn open_obj(& self, oid: daos_obj_id_t, flags: u32) -> Result<daos_handle_t, i32> {
        unsafe {
            let coh = self.daos_conn.get_coh();
            if coh.is_none() {
                return Err(EFAULT);
            }

            let mut oh = daos_handle_t {cookie: 0u64};
            let ret = daos_obj_open(coh.unwrap(), oid, flags, &mut oh, ptr::null_mut());
            if ret != 0 {
                return Err(ENOENT);
            }

            return Ok(oh);
        }
    }

    fn close_obj(hdl: daos_handle_t) -> Result<i32, i32> {
        unsafe {
            let ret = daos_obj_close(hdl, ptr::null_mut());
            if ret != 0 {
                Ok(0)
            } else {
                Err(EFAULT)
            }
        }
    }

    fn get_root_oid(&mut self) -> Result<daos_obj_id_t, i32> {
        fn free_prop(prop: *mut daos_prop_t) {
            unsafe {
                daos_prop_free(prop);
            }
        }
        unsafe {
            let prop = daos_prop_alloc(1);
            if prop.is_null() {
                return Err(ENOMEM);
            }

            let wrapper = RawWrapper {raw_ptr: prop, deallocator: free_prop};
            (*(*wrapper.raw_ptr).dpp_entries).dpe_type = daos_cont_props_DAOS_PROP_CO_ROOTS;

            let coh = self.daos_conn.get_coh();
            if coh.is_none() {
                return Err(EFAULT);
            }

            let ret = daos_cont_query(coh.unwrap(), ptr::null_mut(), prop, ptr::null_mut());
            if ret != 0 {
                return Err(EFAULT);
            }

            let entry = daos_prop_entry_get(prop, daos_cont_props_DAOS_PROP_CO_ROOTS);
            let roots = (*entry).__bindgen_anon_1.dpe_val_ptr as *mut daos_prop_co_roots;
            let root_oid = (*roots).cr_oids[1];

            self.ino_obj_map.insert(ROOT_INODE_NUMBER, InodeInfo {oid: root_oid, parent_oid: daos_obj_id_t {lo: 0u64, hi: 0u64}, name: vec!['/' as u8]});

            Ok(root_oid)
        }
    }

    fn open_root(&mut self) -> Result<daos_handle_t, i32> {
        let root_handle = self.ino_obj_map.get(&ROOT_INODE_NUMBER);
        let root_oid = match root_handle {
            Some(hdl) => hdl.oid,
            None => {
                let result = self.get_root_oid();
                let Ok(oid) = result else {
                    return Err(result.unwrap_err());
                };
                oid
            },
        };
        
        let hdl = self.open_obj(root_oid, DAOS_OO_RO);
        if hdl.is_ok() {
            Ok(hdl.unwrap())
        } else {
            Err(hdl.unwrap_err())
        }
    }

    fn get_inode_entry(&self, parent: daos_handle_t, name: &[u8]) -> Result<InodeEntry, i32> {
        unsafe {
            let mut dkey = daos_key_t {
                // name is immutable but iov_buf is mutable!
                iov_buf: name as *const [u8] as *mut [u8] as *mut c_void,
                iov_buf_len: name.len(),
                iov_len: name.len(),
            };
            let akey = CString::new(INODE_AKEY).unwrap();
            let akey_bytes = akey.to_bytes();
            let mut iod = daos_iod_t {
                iod_name: daos_key_t {
                    iov_buf: akey_bytes as *const [u8] as *mut [u8] as *mut c_void,
                    iov_buf_len: akey_bytes.len(),
                    iov_len: akey_bytes.len(),
                },
                iod_type: daos_iod_type_t_DAOS_IOD_SINGLE,
                iod_size: DAOS_REC_ANY as u64,
                iod_flags: 0,
                iod_nr: 1,
                iod_recxs: ptr::null_mut(),
            };
            let mut inode_buf = Box::new([0u8; 512]);
            let mut sg_iov = d_iov_t {
                iov_buf: inode_buf.as_mut_slice() as *mut [u8] as *mut c_void,
                iov_buf_len: inode_buf.len(),
                iov_len: inode_buf.len(),
            };
            let mut sgl = d_sg_list_t {
                sg_nr: 1,
                sg_nr_out: 0,
                sg_iovs: &mut sg_iov,
            };
            let ret = daos_obj_fetch(parent,
                                     DAOS_TXN_NONE,
                                     DAOS_COND_DKEY_FETCH as u64,
                                     &mut dkey,
                                     1,
                                     &mut iod,
                                     &mut sgl,
                                     ptr::null_mut(),
                                     ptr::null_mut());
            if (-ret) == DER_NONEXIST as i32 {
                return Err(ENOENT);
            } else if ret != 0 {
                return Err(EFAULT);
            }

            if sgl.sg_nr_out == 0 {
                return Err(ENOENT);
            }

            let decoded_result = decode_inode(&inode_buf[0 .. iod.iod_size as usize]);
            if decoded_result.is_err() {
                return Err(EFAULT);
            }

            Ok(decoded_result.unwrap().0)
        }
    }

    fn create_entry(&mut self, parent: daos_handle_t, name: &[u8], inode_entry: &InodeEntry) -> Result<i32, i32> {
        let result = encode_inode(inode_entry);
        let Ok(encode_data) = result else {
            return Err(EFAULT);
        };

        let encoded_bytes: &[u8] = encode_data.bytes();

        unsafe {
            let mut dkey = daos_key_t {
                iov_buf: name as *const [u8] as *mut [u8] as *mut c_void,
                iov_buf_len: name.len(),
                iov_len: name.len(),
            };

            let akey = CString::new(INODE_AKEY).unwrap();
            let akey_bytes = akey.as_bytes();
            let mut iod = daos_iod_t {
                iod_name: daos_key_t {
                    iov_buf: akey_bytes as *const [u8] as *mut [u8] as *mut c_void,
                    iov_buf_len: akey_bytes.len(),
                    iov_len: akey_bytes.len(),
                },
                iod_type: daos_iod_type_t_DAOS_IOD_SINGLE,
                iod_size: encoded_bytes.len() as u64,
                iod_flags: 0,
                iod_nr: 1,
                iod_recxs: ptr::null_mut(),
            };
            let mut sg_iov = d_iov_t {
                iov_buf: encoded_bytes as *const [u8] as *mut [u8] as *mut c_void,
                iov_buf_len: encoded_bytes.len(),
                iov_len: encoded_bytes.len(),
            };
            let mut sgl = d_sg_list_t {
                sg_nr: 1,
                sg_nr_out: 0,
                sg_iovs: &mut sg_iov,
            };

            let ret = daos_obj_update(parent,
                                      DAOS_TXN_NONE,
                                      DAOS_COND_DKEY_UPDATE as u64,
                                      &mut dkey,
                                      1,
                                      &mut iod,
                                      &mut sgl,
                                      ptr::null_mut());
            if ret != 0 {
                return Err(EINVAL);
            }

            Ok(0)
        }
    }

    pub fn start_filesystem() -> Result<i32, i32> {
        let options = vec![MountOption::RW, MountOption::FSName("fsel".to_string())];
        let Some(box_conn) = DAOSConn::new("pool1", "cont1") else {
            return Err(EFAULT);
        };

        let fs = FUSEClient {
            daos_conn: box_conn,
            ino_obj_map: HashMap::new(),
        };

        let mntpt = Path::new("/mnt/fs1");

        let result = fuser::mount2(fs, mntpt, &options);
        let Ok(_) = result else {
            return Err(EFAULT);
        };
        Ok(0)
    }
}

impl Filesystem for FUSEClient {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent != ROOT_INODE_NUMBER {
            reply.error(ENOENT);
            return;
        }

        let root_obj = self.open_root();
        if root_obj.is_err() {
            reply.error(root_obj.unwrap_err());
            return;
        }

        #[allow(unused_variables)]
        let hdl_wrapper = RawTWrapper {obj: root_obj.unwrap(), deallocator: FUSEClient::free_hdl};

        let name_bytes = name.as_bytes();
        let inode = self.get_inode_entry(root_obj.unwrap(), name_bytes);
        if inode.is_err() {
            reply.error(inode.unwrap_err());
            return;
        }

        let inode_entry = inode.unwrap();
        let file_attr = FileAttr {
            ino: inode_entry.inum,
            size: inode_entry.chunk_size,
            blocks: 0,
            atime: UNIX_EPOCH.add(Duration::from_secs(inode_entry.atime)),
            mtime: UNIX_EPOCH.add(Duration::from_secs(inode_entry.mtime)),
            ctime: UNIX_EPOCH.add(Duration::from_secs(inode_entry.ctime)),
            crtime: UNIX_EPOCH.add(Duration::from_secs(inode_entry.crtime)),
            kind: FileType::RegularFile,
            perm: 0o444,
            nlink: 1,
            uid: inode_entry.uid,
            gid: inode_entry.gid,
            rdev: 0,
            flags: 0,
            blksize: 0,
        };

        reply.entry(&Duration::ZERO, &file_attr, 0);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == ROOT_INODE_NUMBER {
            let a = FileAttr {
                ino: ROOT_INODE_NUMBER,
                size: 0,
                blocks: 0,
                atime: UNIX_EPOCH,
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: FileType::Directory,
                perm: 0o555,
                nlink: 2,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
                blksize: 0,
            };
            reply.attr(&Duration::ZERO, &a);
            return;
        }
        
        let ino_info = self.ino_obj_map.get(&ino);
        match ino_info {
            Some(info) => {
                let obj_hdl = self.open_obj(info.parent_oid, DAOS_OO_RO);
                if obj_hdl.is_err() {
                    reply.error(obj_hdl.unwrap_err());
                    return;
                }

                let inode = self.get_inode_entry(obj_hdl.unwrap(), info.name.as_slice());
                if inode.is_err() {
                    reply.error(inode.unwrap_err());
                    return;
                }

                let inode_entry = inode.unwrap();
                let file_attr: FileAttr = inode_entry.into();

                reply.attr(&Duration::ZERO, &file_attr);
            },
            None => {
                reply.error(ENOENT);
            },
        }
    }

    fn readdir(&mut self, _req: &Request, _ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        let result = self.open_root();
        let Ok(obj_hdl) = result else {
            reply.error(result.unwrap_err());
            return;
        };

        #[allow(unused_variables)]
        let hdl_wrapper = RawTWrapper {obj: obj_hdl, deallocator: FUSEClient::free_hdl};

        const KEY_DESC_NUM: usize = 16;
        const KEY_DESC_BUF_SIZE: usize = 256;
        unsafe {
            let mut kds: [daos_key_desc_t; KEY_DESC_NUM] = [daos_key_desc_t {kd_key_len: 0, kd_val_type: 0}; KEY_DESC_NUM];
            let mut anchor: daos_anchor_t = daos_anchor_t {da_type: 0u16, da_shard: 0u16, da_flags: 0u32, da_sub_anchors: 0u64, da_buf: [0u8; 104]};
            let mut key_buf: [u8; KEY_DESC_BUF_SIZE] = [0u8; KEY_DESC_BUF_SIZE];
            let mut key_idx: i64 = 0;
            while !daos_anchor_is_eof(&anchor) {
                let mut num_res: u32 = KEY_DESC_NUM as u32;
                let mut sg_iov: d_iov_t = d_iov_t {
                    iov_buf: &mut key_buf as *mut [u8] as *mut c_void,
                    iov_buf_len: KEY_DESC_BUF_SIZE,
                    iov_len: KEY_DESC_BUF_SIZE,
                };
                let mut sgl: d_sg_list_t = d_sg_list_t {
                    sg_nr: 1,
                    sg_nr_out: 0,
                    sg_iovs: &mut sg_iov,
                };

                let ret = daos_obj_list_dkey(obj_hdl,
                                             DAOS_TXN_NONE,
                                             &mut num_res,
                                             &mut kds[0usize],
                                             &mut sgl,
                                             &mut anchor,
                                             ptr::null_mut());
                if ret != 0 {
                    reply.error(EFAULT);
                    return;
                }

                let mut key_offset = 0u64;
                for i in 0u32 .. num_res {
                    let key_end = key_offset + kds[i as usize].kd_key_len;
                    let key = &key_buf[key_offset as usize .. key_end as usize];
                    if key_idx >= offset {
                        let result = self.get_inode_entry(obj_hdl, key);
                        let Ok(inode_entry) = result else {
                            reply.error(result.unwrap_err());
                            return;
                        };
                        if !self.ino_obj_map.contains_key(&inode_entry.inum) {
                            let parent = self.ino_obj_map.get(&ROOT_INODE_NUMBER);
                            self.ino_obj_map.insert(inode_entry.inum, InodeInfo {
                                oid: daos_obj_id_t {
                                    lo: inode_entry.oid_lo,
                                    hi: inode_entry.oid_hi
                                },
                                parent_oid: parent.unwrap().oid,
                                name: Vec::from(key),
                            });
                        }
                        if reply.add(inode_entry.inum, key_idx, FileType::RegularFile, OsStr::from_bytes(key)) {
                            reply.ok();
                            return;
                        }
                    }
                    key_offset += kds[i as usize].kd_key_len;
                    key_idx += 1;
                }
            }

            reply.ok();
        }
    }

    fn create(&mut self, _req: &Request, parent: u64, name: &OsStr, mode_in: u32, _umask: u32, _flags: i32, reply: ReplyCreate) {
        if parent != ROOT_INODE_NUMBER {
            reply.error(EOPNOTSUPP);
            return;
        }

        if mode_in & libc::S_IFMT as u32 != libc::S_IFREG {
            reply.error(EOPNOTSUPP);
            return;
        }

        let result = self.open_root();
        let Ok(root_hdl) = result else {
            reply.error(result.unwrap_err());
            return;
        };

        #[allow(unused_variables)]
        let hdl_wrapper = RawTWrapper {obj: root_hdl, deallocator: FUSEClient::free_hdl};

        let next_ino = alloc_inum();
        let new_entry = InodeEntry {
            mode: mode_in,
            oid_lo: 0,
            oid_hi: 0,
            atime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            mtime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            ctime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            crtime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            uid: 0,
            gid: 0,
            inum: next_ino,
            chunk_size: 0,
        };

        let result = self.create_entry(root_hdl, name.as_bytes(), &new_entry);
        let Ok(_) = result else {
            reply.error(result.unwrap_err());
            return;
        };

        reply.created(&Duration::new(0, 0), &new_entry.into(), 0, next_ino, 0);
    }

    fn open(&mut self, _req: &Request, ino: u64, _flags: i32, reply: ReplyOpen) {
        reply.opened(ino, FOPEN_DIRECT_IO | FOPEN_NONSEEKABLE);
    }

    fn release(&mut self, _req: &Request, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        reply.ok();
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, _offset: i64, _size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        let str = format!("ino = {}", ino);
        reply.data(str.as_ref());
    }
}
