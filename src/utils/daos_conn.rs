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

use std::ffi::CString;
use std::option::Option;
use std::option::Option::None;
use std::option::Option::Some;
use std::ptr;
use std::sync::Once;
use crate::bindings::daos::DAOS_COO_RW;
use crate::bindings::daos::DAOS_PC_RW;
use crate::bindings::daos::daos_cont_close;
use crate::bindings::daos::daos_cont_open2;
use crate::bindings::daos::daos_handle_t;
use crate::bindings::daos::daos_init;
use crate::bindings::daos::daos_pool_connect2;
use crate::bindings::daos::daos_pool_disconnect;

#[derive(Debug)]
pub struct DAOSConn {
    poh: daos_handle_t,
    coh: daos_handle_t,
    valid_poh: bool,
    valid_coh: bool,
}

static mut INIT_RES: i32 = 0;
static INIT_DAOS: Once = Once::new();

impl DAOSConn {
    pub fn new(pool_name: &str, cont_name: &str) -> Option<Box<DAOSConn>> {
        let mut box_conn = Box::new(DAOSConn{ poh: daos_handle_t{ cookie: 0u64 }, coh: daos_handle_t{ cookie: 0u64 }, valid_poh: false, valid_coh: false });
        let res = box_conn.connect(pool_name, cont_name);
        if res != 0 {
            None
        } else {
            Some(box_conn)
        }
    }

    pub fn get_poh(& self) -> Option<daos_handle_t> {
        match self.valid_poh {
            true => Some(self.poh),
            false => None,
        }
    }

    pub fn get_coh(& self) -> Option<daos_handle_t> {
        match self.valid_coh {
            true => Some(self.coh),
            false => None,
        }
    }
    
    fn connect(&mut self, pool_name: &str, cont_name: &str) -> i32 {
        unsafe {
            INIT_DAOS.call_once(|| {
                INIT_RES = daos_init();
            });
            if INIT_RES != 0 {
                println!("init failed with result {}", INIT_RES);
                return INIT_RES;
            }

            let cpool_name = CString::new(pool_name).expect("create c pool_name failed");
            let ccont_name = CString::new(cont_name).expect("create c cont_name failed");

            let res = daos_pool_connect2(cpool_name.as_ptr(),
                                         ptr::null_mut(),
                                         DAOS_PC_RW,
                                         &mut self.poh,
                                         ptr::null_mut(),
                                         ptr::null_mut());
            if res != 0 {
                println!("daos_pool_connect2 failed with result {}", res);
                return res as i32;
            } else {
                self.valid_poh = true;
            }

            let res = daos_cont_open2(self.poh,
                                      ccont_name.as_ptr(),
                                      DAOS_COO_RW,
                                      &mut self.coh,
                                      ptr::null_mut(),
                                      ptr::null_mut());
            if res != 0 {
                println!("daos_cont_open2 failed with result {}", res);
                return res as i32;
            } else {
                self.valid_coh = true;
            }

            0
        }
    }
}

impl Drop for DAOSConn {
    fn drop(&mut self) {
        unsafe {
            if self.valid_coh {
                println!("cont handle {} is closed", self.coh.cookie);
                daos_cont_close(self.coh, ptr::null_mut());
                self.valid_coh = false;
            }
            if self.valid_poh {
                println!("pool handle {} is closed", self.poh.cookie);
                daos_pool_disconnect(self.poh, ptr::null_mut());
                self.valid_poh = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daos_conn() -> () {
        let pool_name: &'static str = "pool1";
        let cont_name: &'static str = "cont1";

        let daos_conn = DAOSConn::new(pool_name, cont_name);

        match daos_conn {
            Some(x) => println!("get some daos conn"),
            None => println!("connect to daos conn failed"),
        }
    }
}
