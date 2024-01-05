use anyhow::anyhow;
use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use crate::{block::Block, errors::Result, transaction::Transaction, utxoset::UTXOSet};

const KNOWN_NODE: &str = "localhost:3000";
const CMD_LENGTH: usize = 12;
const VERSION: u32 = 1;

pub struct Server {
    node_addr: String,
    miner_addr: String,
    inner: Arc<Mutex<ServerInner>>,
}

struct ServerInner {
    known_nodes: HashSet<String>,
    utxo: UTXOSet,
    blocks_in_transit: Vec<String>,
    mempool: HashMap<String, Transaction>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BlockMsg {
    addr_from: String,
    block: Block,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GetBlocksMsg {
    addr_from: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GetDataMsg {
    addr_from: String,
    kind: String,
    id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct InvMsg {
    addr_from: String,
    kind: String,
    items: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TxMsg {
    addr_from: String,
    transaction: Transaction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct VersionMsg {
    addr_from: String,
    version: u32,
    best_height: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum ServerMessage {
    Addr(Vec<String>),
    Version(VersionMsg),
    Tx(TxMsg),
    GetData(GetDataMsg),
    GetBlocks(GetBlocksMsg),
    Inv(InvMsg),
    Block(BlockMsg),
}

impl Server {
    pub fn new(port: &str, miner_addr: &str, utxo: UTXOSet) -> Result<Self> {
        let mut node_set = HashSet::new();
        node_set.insert(KNOWN_NODE.to_string());

        Ok(Self {
            node_addr: format!("localhost:{}", port),
            miner_addr: miner_addr.to_string(),
            inner: Arc::new(Mutex::new(ServerInner {
                known_nodes: node_set,
                utxo,
                blocks_in_transit: Vec::new(),
                mempool: HashMap::new(),
            })),
        })
    }

    pub fn start_server(&self) -> Result<()> {
        let srv = Self {
            node_addr: self.node_addr.clone(),
            miner_addr: self.miner_addr.clone(),
            inner: self.inner.clone(),
        };

        thread::spawn(move || {
            // TODO
        });

        let listener = TcpListener::bind(&self.node_addr)?;

        for stream in listener.incoming() {
            let stream = stream?;
            let srv = Self {
                node_addr: self.node_addr.clone(),
                miner_addr: self.miner_addr.clone(),
                inner: self.inner.clone(),
            };

            thread::spawn(move || {
                // TODO
            });
        }

        Ok(())
    }

    pub fn send_transaction(tx: &Transaction, utxoset: UTXOSet) -> Result<()> {
        let srv = Self::new("7000", "", utxoset)?;
        // TODO
        Ok(())
    }

    /*
       ====================
        internal functions
       ====================
    */
    fn remove_node(&self, addr: &str) {
        self.inner.lock().unwrap().known_nodes.remove(addr);
    }

    fn add_nodes(&self, addr: &str) {
        self.inner
            .lock()
            .unwrap()
            .known_nodes
            .insert(String::from(addr));
    }

    fn get_known_nodes(&self) -> HashSet<String> {
        self.inner.lock().unwrap().known_nodes.clone()
    }

    fn node_is_known(&self, addr: &str) -> bool {
        self.inner.lock().unwrap().known_nodes.get(addr).is_some()
    }

    fn replace_in_transit(&self, hashs: Vec<String>) {
        let bit = &mut self.inner.lock().unwrap().blocks_in_transit;
        bit.clone_from(&hashs);
    }

    fn get_in_transit(&self) -> Vec<String> {
        self.inner.lock().unwrap().blocks_in_transit.clone()
    }

    fn get_mempool_tx(&self, addr: &str) -> Option<Transaction> {
        match self.inner.lock().unwrap().mempool.get(addr) {
            Some(tx) => Some(tx.clone()),
            None => None,
        }
    }

    fn get_mempool(&self) -> HashMap<String, Transaction> {
        self.inner.lock().unwrap().mempool.clone()
    }

    fn insert_mempool(&self, tx: Transaction) {
        self.inner.lock().unwrap().mempool.insert(tx.id.clone(), tx);
    }

    fn clear_mempool(&self) {
        self.inner.lock().unwrap().mempool.clear()
    }

    fn get_best_height(&self) -> Result<u32> {
        self.inner.lock().unwrap().utxo.blockchain.get_best_height()
    }

    fn get_block_hashs(&self) -> Vec<String> {
        self.inner.lock().unwrap().utxo.blockchain.get_block_hashs()
    }

    fn get_block(&self, block_hash: &str) -> Result<Block> {
        self.inner
            .lock()
            .unwrap()
            .utxo
            .blockchain
            .get_block(block_hash)
    }

    fn verify_tx(&self, tx: &Transaction) -> Result<bool> {
        self.inner
            .lock()
            .unwrap()
            .utxo
            .blockchain
            .verify_transaction(tx)
    }

    fn add_block(&self, block: Block) -> Result<()> {
        self.inner.lock().unwrap().utxo.blockchain.add_block(block)
    }

    fn mine_block(&self, txs: Vec<Transaction>) -> Result<Block> {
        self.inner.lock().unwrap().utxo.blockchain.mine_block(txs)
    }

    fn utxo_reindex(&self) -> Result<()> {
        self.inner.lock().unwrap().utxo.reindex()
    }

    /* -----------------------------------------------------*/

    fn send_data(&self, addr: &str, data: &[u8]) -> Result<()> {
        if addr == &self.node_addr {
            return Ok(());
        }
        let mut stream = match TcpStream::connect(addr) {
            Ok(s) => s,
            Err(_) => {
                self.remove_node(addr);
                return Ok(());
            }
        };

        stream.write(data)?;

        Ok(())
    }

    fn request_blocks(&self) -> Result<()> {
        for node in self.get_known_nodes() {
            self.send_get_blocks(&node)?
        }
        Ok(())
    }

    fn send_block(&self, addr: &str, b: &Block) -> Result<()> {
        let data = BlockMsg {
            addr_from: self.node_addr.clone(),
            block: b.clone(),
        };
        let data = serialize(&(cmd_to_bytes("block"), data))?;
        self.send_data(addr, &data)
    }

    fn send_addr(&self, addr: &str) -> Result<()> {
        let nodes = self.get_known_nodes();
        let data = serialize(&(cmd_to_bytes("addr"), nodes))?;
        self.send_data(addr, &data)
    }

    fn send_inv(&self, addr: &str, kind: &str, items: Vec<String>) -> Result<()> {
        let data = InvMsg {
            addr_from: self.node_addr.clone(),
            kind: kind.to_string(),
            items,
        };
        let data = serialize(&(cmd_to_bytes("inv"), data))?;
        self.send_data(addr, &data)
    }

    fn send_get_blocks(&self, addr: &str) -> Result<()> {
        let data = GetBlocksMsg {
            addr_from: self.node_addr.clone(),
        };
        let data = serialize(&(cmd_to_bytes("getblocks"), data))?;
        self.send_data(addr, &data)
    }

    fn send_get_data(&self, addr: &str, kind: &str, id: &str) -> Result<()> {
        let data = GetDataMsg {
            addr_from: self.node_addr.clone(),
            kind: kind.to_string(),
            id: id.to_string(),
        };
        let data = serialize(&(cmd_to_bytes("getdata"), data))?;
        self.send_data(addr, &data)
    }

    pub fn send_tx(&self, addr: &str, tx: &Transaction) -> Result<()> {
        let data = TxMsg {
            addr_from: self.node_addr.clone(),
            transaction: tx.clone(),
        };
        let data = serialize(&(cmd_to_bytes("tx"), data))?;
        self.send_data(addr, &data)
    }

    fn send_version(&self, addr: &str) -> Result<()> {
        let data = VersionMsg {
            addr_from: self.node_addr.clone(),
            best_height: self.get_best_height()?,
            version: VERSION,
        };
        let data = serialize(&(cmd_to_bytes("version"), data))?;
        self.send_data(addr, &data)
    }

    fn handle_version(&self, msg: VersionMsg) -> Result<()> {
        let my_best_height = self.get_best_height()?;
        if my_best_height < msg.best_height {
            self.send_get_blocks(&msg.addr_from)?;
        } else if my_best_height > msg.best_height {
            self.send_version(&msg.addr_from)?;
        }

        self.send_addr(&msg.addr_from)?;

        if !self.node_is_known(&msg.addr_from) {
            self.add_nodes(&msg.addr_from);
        }
        Ok(())
    }

    fn handle_addr(&self, msg: Vec<String>) -> Result<()> {
        for node in msg {
            self.add_nodes(&node);
        }
        //self.request_blocks()?;
        Ok(())
    }

    fn handle_block(&self, msg: BlockMsg) -> Result<()> {
        self.add_block(msg.block)?;

        let mut in_transit = self.get_in_transit();
        if in_transit.len() > 0 {
            let block_hash = &in_transit[0];
            self.send_get_data(&msg.addr_from, "block", block_hash)?;
            in_transit.remove(0);
            self.replace_in_transit(in_transit);
        } else {
            self.utxo_reindex()?;
        }

        Ok(())
    }

    fn handle_inv(&self, msg: InvMsg) -> Result<()> {
        if msg.kind == "block" {
            let block_hash = &msg.items[0];
            self.send_get_data(&msg.addr_from, "block", block_hash)?;

            let mut new_in_transit = Vec::new();
            for b in &msg.items {
                if b != block_hash {
                    new_in_transit.push(b.clone());
                }
            }
            self.replace_in_transit(new_in_transit);
        } else if msg.kind == "tx" {
            let txid = &msg.items[0];
            match self.get_mempool_tx(txid) {
                Some(tx) => {
                    if tx.id.is_empty() {
                        self.send_get_data(&msg.addr_from, "tx", txid)?
                    }
                }
                None => self.send_get_data(&msg.addr_from, "tx", txid)?,
            }
        }
        Ok(())
    }

    fn handle_get_blocks(&self, msg: GetBlocksMsg) -> Result<()> {
        let block_hashs = self.get_block_hashs();
        self.send_inv(&msg.addr_from, "block", block_hashs)?;
        Ok(())
    }

    fn handle_get_data(&self, msg: GetDataMsg) -> Result<()> {
        if msg.kind == "block" {
            let block = self.get_block(&msg.id)?;
            self.send_block(&msg.addr_from, &block)?;
        } else if msg.kind == "tx" {
            let tx = self.get_mempool_tx(&msg.id).unwrap();
            self.send_tx(&msg.addr_from, &tx)?;
        }
        Ok(())
    }

    fn handle_tx(&self, msg: TxMsg) -> Result<()> {
        self.insert_mempool(msg.transaction.clone());

        let known_nodes = self.get_known_nodes();
        if self.node_addr == KNOWN_NODE {
            for node in known_nodes {
                if node != self.node_addr && node != msg.addr_from {
                    self.send_inv(&node, "tx", vec![msg.transaction.id.clone()])?;
                }
            }
        } else {
            let mut mempool = self.get_mempool();
            if mempool.len() >= 1 && !self.miner_addr.is_empty() {
                loop {
                    let mut txs = Vec::new();

                    for (_, tx) in &mempool {
                        if self.verify_tx(tx)? {
                            txs.push(tx.clone());
                        }
                    }

                    if txs.is_empty() {
                        return Ok(());
                    }

                    let cbtx = Transaction::new_coinbase(self.miner_addr.clone(), String::new())?;
                    txs.push(cbtx);

                    for tx in &txs {
                        mempool.remove(&tx.id);
                    }

                    let new_block = self.mine_block(txs)?;
                    self.utxo_reindex()?;

                    for node in self.get_known_nodes() {
                        if node != self.node_addr {
                            self.send_inv(&node, "block", vec![new_block.get_hash()])?;
                        }
                    }

                    if mempool.len() == 0 {
                        break;
                    }
                }
                self.clear_mempool();
            }
        }

        Ok(())
    }

    fn handle_connection(&self, mut stream: TcpStream) -> Result<()> {
        let mut buffer = Vec::new();
        let count = stream.read_to_end(&mut buffer)?;

        let cmd = bytes_to_cmd(&buffer)?;

        match cmd {
            ServerMessage::Addr(data) => self.handle_addr(data)?,
            ServerMessage::Block(data) => self.handle_block(data)?,
            ServerMessage::Inv(data) => self.handle_inv(data)?,
            ServerMessage::GetBlocks(data) => self.handle_get_blocks(data)?,
            ServerMessage::GetData(data) => self.handle_get_data(data)?,
            ServerMessage::Tx(data) => self.handle_tx(data)?,
            ServerMessage::Version(data) => self.handle_version(data)?,
        }

        Ok(())
    }
}

fn cmd_to_bytes(cmd: &str) -> [u8; CMD_LENGTH] {
    let mut data = [0; CMD_LENGTH];
    for (i, d) in cmd.as_bytes().iter().enumerate() {
        data[i] = *d;
    }
    data
}

fn bytes_to_cmd(bytes: &[u8]) -> Result<ServerMessage> {
    let mut cmd = Vec::new();
    let cmd_bytes = &bytes[..CMD_LENGTH];
    let data = &bytes[CMD_LENGTH..];
    for b in cmd_bytes {
        if 0 as u8 != *b {
            cmd.push(*b);
        }
    }

    if cmd == "addr".as_bytes() {
        let data: Vec<String> = deserialize(data)?;
        Ok(ServerMessage::Addr(data))
    } else if cmd == "block".as_bytes() {
        let data: BlockMsg = deserialize(data)?;
        Ok(ServerMessage::Block(data))
    } else if cmd == "inv".as_bytes() {
        let data: InvMsg = deserialize(data)?;
        Ok(ServerMessage::Inv(data))
    } else if cmd == "getblocks".as_bytes() {
        let data: GetBlocksMsg = deserialize(data)?;
        Ok(ServerMessage::GetBlocks(data))
    } else if cmd == "getdata".as_bytes() {
        let data: GetDataMsg = deserialize(data)?;
        Ok(ServerMessage::GetData(data))
    } else if cmd == "tx".as_bytes() {
        let data: TxMsg = deserialize(data)?;
        Ok(ServerMessage::Tx(data))
    } else if cmd == "version".as_bytes() {
        let data: VersionMsg = deserialize(data)?;
        Ok(ServerMessage::Version(data))
    } else {
        Err(anyhow!("unknown command"))
    }
}
