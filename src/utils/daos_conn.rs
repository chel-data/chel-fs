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

use std::ptr;
use std::ffi::CString;
use crate::bindings::daos::daos_handle_t;
use crate::bindings::daos::daos_pool_connect2;
use crate::bindings::daos::daos_pool_disconnect;
use crate::bindings::daos::daos_cont_open2;
use crate::bindings::daos::daos_cont_close;
use crate::bindings::daos::DAOS_PC_RW;
use crate::bindings::daos::DAOS_COO_RW;

#[derive(Debug)]
struct DAOSConn {
    poh: daos_handle_t,
    coh: daos_handle_t,
    valid_poh: bool,
    valid_coh: bool,
}

impl DAOSConn {
    fn new() -> Box<DAOSConn> {
        Box::new(DAOSConn{ poh: daos_handle_t{ cookie: 0u64 }, coh: daos_handle_t{ cookie: 0u64 }, valid_poh: false, valid_coh: false })
    }

    fn connect(&mut self, pool_name: &str, cont_name: &str) -> i32 {
        unsafe {
            let cpool_name = CString::new(pool_name).expect("create c pool_name failed");
            let ccont_name = CString::new(cont_name).expect("create c cont_name failed");

            let res = daos_pool_connect2(cpool_name.as_ptr(),
                                         ptr::null_mut(),
                                         DAOS_PC_RW,
                                         &mut self.poh,
                                         ptr::null_mut(),
                                         ptr::null_mut());
            if res != 0 {
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
                daos_cont_close(self.coh, ptr::null_mut());
            }
            if self.valid_poh {
                daos_pool_disconnect(self.poh, ptr::null_mut());
            }
        }
    }
}
