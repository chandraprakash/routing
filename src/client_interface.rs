// Copyright 2015 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

use error::{ResponseError, RoutingError};
use data::Data;
use NameType;

#[deny(missing_docs)]
/// The Interface trait introduces the methods expected to be implemented by the user
/// of RoutingClient
pub trait Interface : Sync + Send {
    /// consumes data in response or handles the error
    fn handle_get_response(&mut self, data_location : NameType, data : Data);

    /// handles the result of a put request
    fn handle_put_response(&mut self, response_error : ResponseError,
                                      request_data   : Data);

    /// handles the result of a post request
    fn handle_post_response(&mut self, response_error : ResponseError,
                                       request_data   : Data);

    /// handles the result of a delete request
    fn handle_delete_response(&mut self, response_error : ResponseError,
                                         request_data   : Data);
}
