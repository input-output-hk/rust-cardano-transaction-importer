use cardano::block;
use cardano::block::block::{Block, BlockHeader};
use cardano::block::date::BlockDate;
use cardano::block::types::HeaderHash;

use crate::EpochId;

#[derive(Clone)]
pub struct HttpBridge {
    url: String,
}

impl HttpBridge {
    pub fn first_unstable_epoch(&self, epoch_stability_depth: usize) -> std::result::Result<EpochId, Box<std::error::Error>> {
        let tip = self.get_tip()?;
        let date = tip.get_blockdate();
        let first_unstable_epoch = date.get_epochid()
            - match date {
                BlockDate::Boundary(_) => 1,
                BlockDate::Normal(d) => {
                    if d.slotid as usize <= epoch_stability_depth {
                        1
                    } else {
                        0
                    }
                }
            };
        Ok(first_unstable_epoch)
    }
}

use std::result::Result;
pub trait HttpBridgeApi {
    fn new(url: String) -> Self;

    fn get_tip(&self) -> Result<BlockHeader, reqwest::Error>;

    fn get_block(&self, blockid: &HeaderHash) -> Result<Block, reqwest::Error>;

    fn get_epoch(&self, id: EpochId) -> Result<Vec<u8>, reqwest::Error>;

}

impl HttpBridgeApi for HttpBridge {
    fn new(url: String) -> Self {
        HttpBridge {
            url,
        }
    }

    fn get_tip(&self) -> std::result::Result<BlockHeader, reqwest::Error> {
        let query = format!("{}tip", self.url);
        let mut resp = reqwest::get(&query)?;
        let mut buf: Vec<u8> = vec![];
        resp.copy_to(&mut buf)?;

        let raw_header_block = block::RawBlockHeader(buf);
        Ok(raw_header_block.decode().unwrap())
    }

    fn get_block(&self, blockid: &HeaderHash) -> Result<Block, reqwest::Error> {
        let query = format!("{}block/{}", self.url, blockid);
        let mut resp = reqwest::get(&query)?;
        let mut buf: Vec<u8> = vec![];
        resp.copy_to(&mut buf)?;

        let raw_block = block::RawBlock(buf);
        Ok(raw_block.decode().unwrap())
    }

    fn get_epoch(&self, id: EpochId) -> Result<Vec<u8>, reqwest::Error>
    {
        let query = format!("{}epoch/{}", self.url, id);
        let mut resp = reqwest::get(&query)?;
        let mut buf: Vec<u8> = vec![];
        resp.copy_to(&mut buf)?;

        Ok(buf)
    }
}
