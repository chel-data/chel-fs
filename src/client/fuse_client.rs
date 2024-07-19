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
    DAOS_OO_RO,
    DAOS_REC_ANY,
    DAOS_TXN_NONE,
    DER_NONEXIST,
    daos_cont_props_DAOS_PROP_CO_ROOTS,
    daos_cont_query,
    daos_handle_t,
    daos_iod_t,
    daos_iod_type_t_DAOS_IOD_SINGLE,
    daos_key_t,
    daos_obj_fetch,
    daos_obj_id_t,
    daos_obj_open,
    daos_obj_close,
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
    ReplyAttr,
    ReplyData,
    ReplyDirectory,
    ReplyEmpty,
    ReplyEntry,
    ReplyOpen,
    Request
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
    ptr,
    result::Result,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, UNIX_EPOCH},
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
        if !self.raw_ptr.is_null() {
            (self.deallocator)(self.raw_ptr);
            self.raw_ptr = ptr::null_mut();
        }
    }
}

#[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
struct InodeEntry{
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

#[derive(Debug)]
struct InodeInfo {
    oid: daos_obj_id_t,
    parent_oid: daos_obj_id_t,
    name: Vec<u8>,
}

struct FUSEClient {
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

    fn close_obj(& self, hdl: daos_handle_t) -> Result<i32, i32> {
        unsafe {
            let ret = daos_obj_close(hdl, ptr::null_mut());
            if ret != 0 {
                Ok(0)
            } else {
                Err(EFAULT)
            }
        }
    }
    
    fn open_root(&mut self) -> Result<daos_handle_t, i32> {
        fn free_prop(prop: *mut daos_prop_t) {
            unsafe {
                daos_prop_free(prop);
            }
        }

        let root_handle = self.ino_obj_map.get(&ROOT_INODE_NUMBER);
        let root_oid = match root_handle {
            Some(hdl) => hdl.oid,
            None => unsafe {
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

                root_oid
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
}

impl Filesystem for FUSEClient {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent != ROOT_INODE_NUMBER {
            reply.error(ENOENT);
            return;
        }

        let root_obj = self.open_root();
        if root_obj.is_err() {
            reply.error(root_obj.unwrap_err());
            return;
        }

        let name_bytes = name.as_bytes();
        let inode = self.get_inode_entry(root_obj.unwrap(), name_bytes);
        if inode.is_err() {
            self.close_obj(root_obj.unwrap());
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
        self.close_obj(root_obj.unwrap());
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

                reply.attr(&Duration::ZERO, &file_attr);
            },
            None => {
                reply.error(ENOENT);
            },
        }
    }

    fn readdir(&mut self, _req: &Request, _ino: u64, _fh: u64, _offset: i64, mut reply: ReplyDirectory) {
        
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
