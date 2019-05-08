use rusqlite::params;

use cardano::address::ExtendedAddr;
use cardano::block::chain_state::Utxos;
use cardano::tx::Tx;
use cardano::tx::{TxOut, TxoPointer};

use cardano::block::block::Block;

use crate::types::{Input, Output, Transaction};
use cardano::block::types::HeaderHash;
use rusqlite::Connection;
use std::str::FromStr;

use cardano::block;
use cardano::block::types::EpochId;
use storage_units::packfile::Reader;

pub fn prepare_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
            begin;
            create table if not exists tx (
                id integer primary key,
                txid text unique not null
            );
            create table if not exists address (
                id integer primary key,
                address text unique not null
            );
            create table if not exists txs_by_address (
                id integer primary key,
                tx integer not null references tx(id),
                address integer not null references address(id),
                unique(tx, address)
            );
            create index txs_by_address_address on txs_by_address (address);
            create table if not exists input (
                id integer primary key,
                tx integer not null references tx(id),
                source_tx integer not null references tx(id),
                offset integer not null
            );
            create table if not exists output (
                id integer primary key,
                tx integer not null references tx(id),
                address integer not null references address(id),
                value integer not null,
                offset integer not null
            );
            create index if not exists output_index_tx on output(tx);
            create table if not exists block (
                id text primary key,
                next text
            );
            create table if not exists last_block (
                id integer primary key check (id = 0),
                block text
            );
            commit;
        "#,
    )?;
    Ok(())
}

pub fn insert_tx(conn: &Connection, tx: Tx) -> rusqlite::Result<()> {
    let hash = format!("{}", tx.id());
    let inputs = tx.inputs;
    let outputs = tx.outputs;

    conn.execute(
        "insert into tx (id, txid)
        values (NULL, ?1)",
        params![hash],
    )?;

    let txid = conn.last_insert_rowid();

    for (idx, output) in outputs.iter().enumerate() {
        match add_output(&conn, txid, output, idx as u32) {
            Ok(_) => (),
            Err(e) => {
                error!("Error inserting output: {}", e);
                return Err(e);
            }
        };
    }

    for input in inputs {
        match add_input(&conn, txid, &input) {
            Ok(_) => (),
            Err(e) => {
                error!("Error inserting input: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

fn add_input(conn: &Connection, txid: i64, input: &TxoPointer) -> rusqlite::Result<()> {
    let source_tx: i64 = conn.query_row(
        "SELECT id FROM tx WHERE tx.txid=?1",
        params![format!("{}", input.id)],
        |row| row.get(0),
    )?;

    conn.execute(
        "insert into input (id, tx, source_tx, offset)
        values (NULL, ?1, ?2, ?3)",
        params![txid, source_tx, input.index],
    )?;

    conn.execute(
        "insert or ignore into txs_by_address
        select null, ?3, output.address from 
            tx join output
            on tx.id = output.tx
        where
            tx.txid = ?1 and
            output.offset = ?2
        ",
        params![format!("{}", input.id), input.index, txid],
    )?;

    Ok(())
}

fn add_output(
    conn: &rusqlite::Connection,
    txid: i64,
    output: &TxOut,
    idx: u32,
) -> rusqlite::Result<()> {
    let address = format!("{}", output.address);

    conn.execute(
        "insert or ignore into address (id, address)
        values (NULL, ?1)
        ",
        params![address],
    )?;

    let address_id: i64 = conn.query_row(
        "SELECT rowid FROM address WHERE address=?1",
        params![address],
        |row| row.get(0),
    )?;

    conn.execute(
        "insert into output (id, tx, address, value, offset)
        values (NULL, ?1, ?2, ?3, ?4)
        ",
        params!(txid, address_id, u64::from(output.value) as i64, idx as u32),
    )?;

    conn.execute(
        "insert or ignore into txs_by_address (id, tx, address)
        values (NULL, ?1, ?2)
        ",
        params!(txid, address_id),
    )?;

    Ok(())
}

pub fn apply_initial_state(conn: &mut Connection, utxos: &Utxos) -> rusqlite::Result<()> {
    if let Some(_) = last_applied_block(&conn)? {
        return Ok(());
    }

    let transaction = conn.transaction().unwrap();

    for (k, v) in utxos {
        transaction.execute(
            "insert into tx (id, txid)
            values (NULL, ?1)",
            params![format!("{}", k.id)],
        )?;

        let txid = transaction.last_insert_rowid();

        add_output(&transaction, txid, &v, k.index)?;
    }

    transaction.commit()?;
    Ok(())
}

pub fn inputs(conn: &Connection, tx: i64) -> rusqlite::Result<Vec<Input>> {
    let mut inputs_stmt = conn
        .prepare(
            "SELECT input.offset, source_tx.txid
    FROM 
    tx 
    JOIN
    input
        ON input.tx = tx.id
    JOIN
    tx as source_tx
        ON input.source_tx = source_tx.id
    WHERE
        tx.id = ?1",
        )
        .unwrap();

    let inputs_iter = inputs_stmt
        .query_map(params![tx], |row| {
            Ok(Input {
                index: row.get(0)?,
                id: row.get(1)?,
            })
        })
        .unwrap();

    let inputs: rusqlite::Result<Vec<Input>> =
        inputs_iter.map(|input| Ok(input?)).collect();

    Ok(inputs?)
}

pub fn outputs(conn: &Connection, tx: i64) -> rusqlite::Result<Vec<Output>> {
    let mut outputs_stmt = conn
        .prepare(
            "
    SELECT address.address,
        output.value
    FROM   tx
        JOIN output
            ON output.tx = tx.id
        JOIN address
            ON address.id = output.address  
    WHERE
        tx.id = ?1
    ORDER BY
        output.offset
    ",
        )
        .unwrap();

    let outputs_iter = outputs_stmt
        .query_map(params![tx], |row| {
            Ok(Output {
                address: row.get(0)?,
                value: row.get(1)?,
            })
        })
        .unwrap();

    let outputs: rusqlite::Result<Vec<Output>> =
        outputs_iter.map(|output| Ok(output?)).collect();

    Ok(outputs?)
}

pub fn transaction(conn: &Connection, txid: String) -> rusqlite::Result<Transaction> {
    let tx: i64 = conn
        .query_row(
            "SELECT id FROM tx WHERE 
                txid=?1
            ",
            params![txid],
            |row| row.get(0),
        )?;

    Ok(Transaction {
        txid,
        inputs: inputs(conn, tx)?,
        outputs: outputs(conn, tx)?,
    })
}

pub fn transactions_of(
    conn: &Connection,
    address: ExtendedAddr,
) -> rusqlite::Result<Vec<Transaction>> {
    let mut transactions_stmt = conn
        .prepare(
            "SELECT tx.id, tx.txid 
        FROM tx JOIN txs_by_address
        ON tx.id = txs_by_address.tx
        JOIN address
        ON txs_by_address.address = address.id
        WHERE address.address = ?1
    ",
        )
        .unwrap();
    let transaction_iter = transactions_stmt
        .query_map(params![format!("{}", address)], |row| {
            let id: i64 = row.get(0)?;
            let txid: String = row.get(1)?;
            Ok((id, txid))
        })
        .unwrap();

    transaction_iter
        .map(|transaction| {
            let (tx, txid) = transaction?;

            Ok(Transaction {
                txid,
                inputs: inputs(conn, tx)?,
                outputs: outputs(conn, tx)?,
            })
        })
        .collect()
}

pub fn apply_block(conn: &Connection, block: &Block) -> rusqlite::Result<()> {
    if let Some(payload) = block.get_transactions() {
        for tx_aux in payload {
                insert_tx(conn, tx_aux.tx)?;
        }
    }

    let hash = block.header().compute_hash();

    match conn.execute(
        "insert or replace into last_block(id, block)
        values (0, ?1)",
        params![format!("{}", hash)],
    ) {
        Ok(_) => (),
        Err(e) => error!("Couldn't update last_block to {}: {}", hash, e),
    }

    Ok(())
}

pub fn last_applied_block(conn: &Connection) -> rusqlite::Result<Option<HeaderHash>> {
    let last_block: Option<String> = match conn.query_row(
        "SELECT block FROM last_block WHERE id = 0",
        rusqlite::NO_PARAMS,
        |row| row.get(0),
    ) {
        Ok(string) => Some(string),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(e),
    };
    Ok(last_block.map(|hash| HeaderHash::from_str(&hash).unwrap()))
}

pub fn next_block(conn: &Connection, block: HeaderHash) -> rusqlite::Result<Option<HeaderHash>> {
    let result: Option<String> = conn.query_row(
        "SELECT next FROM block WHERE id = ?1",
        params![format!("{}", block)],
        |row| row.get(0),
    )?;

    Ok(result.map(|h| HeaderHash::from_str(&h).unwrap()))
}

pub fn update_block_index<F>(
    conn: &mut Connection,
    to: HeaderHash,
    get_previous: F,
) -> crate::types::Result<()>
where
    F: Fn(HeaderHash) -> std::result::Result<HeaderHash, crate::types::Error>,
{
    info!("Updating block index");
    let transaction = conn.transaction().unwrap();

    let last_block: String = transaction.query_row(
        "SELECT id FROM block WHERE next IS NULL",
        rusqlite::NO_PARAMS,
        |row| row.get(0),
    )?;

    let mut next = None;
    let mut cursor = to;

    loop {
        transaction.execute(
            "insert or replace into block(id, next)
            values (?1, ?2)
            ",
            params![format!("{}", &cursor), next.map(|h| format!("{}", h))],
        )?;

        info!("Inserting block {}", format!("{}", &cursor));

        if format!("{}", cursor) == last_block {
            break;
        }

        next = Some(cursor.clone());
        cursor = get_previous(cursor)?;
    }

    transaction.commit()?;
    Ok(())
}

use std::time::{Instant};
pub fn sync_from_epochs<F>(
    conn: &mut Connection,
    first_unstable_epoch: EpochId,
    get_epoch: F,
) -> rusqlite::Result<()>
where
    F: Fn(EpochId) -> Vec<u8>,
{
    for i in 0..first_unstable_epoch {

        let transaction = conn.transaction().unwrap();

        info!("Epoch: {}", i);

        let epoch = get_epoch(i);
        let mut reader = Reader::init(epoch.as_slice()).unwrap();

        let mut hashes = vec![];

        let now = Instant::now();

        while let Some(b) = reader.next_block().unwrap() {
            let block = block::RawBlock(b).decode().unwrap();

            apply_block(&transaction, &block)?;

            let hash = block.header().compute_hash();
            hashes.push(format!("{}", hash));
        }

        match transaction.query_row(
            "SELECT id FROM block WHERE next IS NULL",
            rusqlite::NO_PARAMS,
            |row| row.get(0),
        ) {
            Ok(hash) => {
                let hash: String = hash;
                transaction.execute(
                    "insert or replace into block(id, next)
                values (?1, ?2)
                ",
                    params![format!("{}", hash), hashes[0]],
                )?;
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => (),
            Err(e) => return Err(e),
        };

        for i in 0..hashes.len() {
            let block = &hashes[i];
            let next = hashes.get(i + 1);

            transaction.execute(
                "insert or replace into block(id, next)
                values (?1, ?2)
                ",
                params![format!("{}", block), next.map(|h| format!("{}", h))],
            )?;
        }

        transaction.commit()?;
        info!("Finished epoch {} in {}", i, now.elapsed().as_millis());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano::address::ExtendedAddr;
    use cardano::coin::Coin;
    use cardano::hash;
    use cardano::util::base58;
    use cardano::util::try_from_slice::TryFromSlice;
    use std::collections::BTreeMap;
    use std::str::FromStr;

    #[test]
    fn test_block_index_update() {
        let mut conn = Connection::open(":memory:").unwrap();
        prepare_schema(&conn).unwrap();

        let next = None;
        let initial = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91842",
        )
        .unwrap();

        let hash1 = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91843",
        )
        .unwrap();
        let hash2 = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91844",
        )
        .unwrap();

        conn.execute(
            "insert or replace into block(id, next)
            values (?1, ?2)
            ",
            params![
                format!("{}", initial),
                next.map(|h: HeaderHash| format!("{}", h))
            ],
        )
        .unwrap();

        update_block_index(&mut conn, hash2.clone(), |h| {
            if h == hash2 {
                hash1.clone()
            } else {
                initial.clone()
            }
        })
        .unwrap();

        let second: String = conn
            .query_row(
                "SELECT next FROM block WHERE id = ?1",
                params![format!("{}", initial)],
                |row| row.get(0),
            )
            .unwrap();

        assert!(second == format!("{}", hash1));

        let third: String = conn
            .query_row(
                "SELECT next FROM block WHERE id = ?1",
                params![format!("{}", hash1)],
                |row| row.get(0),
            )
            .unwrap();

        assert!(third == format!("{}", hash2));

        let fourth: Option<String> = conn
            .query_row(
                "SELECT next FROM block WHERE id = ?1",
                params![format!("{}", hash2)],
                |row| row.get(0),
            )
            .unwrap();

        assert!(fourth.is_none());
    }

    #[test]
    fn test_blocks_to_apply() {
        let mut conn = Connection::open(":memory:").unwrap();

        prepare_schema(&conn).unwrap();

        let next = None;
        let initial = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91842",
        )
        .unwrap();

        let hash1 = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91843",
        )
        .unwrap();

        let hash2 = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91844",
        )
        .unwrap();

        conn.execute(
            "insert or replace into block(id, next)
                values (?1, ?2)
                ",
            params![
                format!("{}", initial),
                next.map(|h: HeaderHash| format!("{}", h))
            ],
        )
        .unwrap();

        conn.execute(
            "insert or replace into last_block(id, block)
                values (0, ?1)
                ",
            params![format!("{}", initial)],
        )
        .unwrap();

        update_block_index(&mut conn, hash2.clone(), |h| {
            if h == hash2 {
                hash1.clone()
            } else {
                initial.clone()
            }
        })
        .unwrap();

        let block1 = last_applied_block(&conn).unwrap().unwrap();
        assert!(block1 == initial);
        let block2 = next_block(&conn, block1).unwrap().unwrap();
        assert!(block2 == hash1);
        let block3 = next_block(&conn, block2).unwrap().unwrap();
        assert!(block3 == hash2);
    }

    #[test]
    fn test_initial_state() {
        let mut conn = Connection::open(":memory:").unwrap();
        prepare_schema(&conn).unwrap();

        let mut utxos = BTreeMap::new();

        let addr_str = "Ae2tdPwUPEZKmwoy3AU3cXb5Chnasj6mvVNxV1H11997q3VW5ihbSfQwGpm";
        let bytes = base58::decode(addr_str).unwrap();
        let address = ExtendedAddr::try_from_slice(&bytes).unwrap();
        let id = hash::Blake2b256::new(&[0]);

        let value = 10000;

        utxos.insert(
            TxoPointer { id, index: 0 },
            TxOut {
                address,
                value: Coin::new(value).unwrap(),
            },
        );

        let initial = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91842",
        )
        .unwrap();
        apply_initial_state(&mut conn, &utxos).unwrap();

        let address_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM address WHERE address=?1",
                params![addr_str],
                |row| row.get(0),
            )
            .unwrap();

        assert!(address_rowid > 0);

        let tx_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM tx WHERE txid=?1",
                params![format!("{}", id)],
                |row| row.get(0),
            )
            .unwrap();

        assert!(tx_rowid > 0);

        let output_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM output WHERE 
                            tx=?1 AND
                            address=?2 AND
                            offset=?3 AND
                            value=?4
                        ",
                params![tx_rowid, address_rowid, 0, value as i64],
                |row| row.get(0),
            )
            .unwrap();

        assert!(output_rowid > 0);

        let tx_by_address: i64 = conn
            .query_row(
                "SELECT rowid FROM txs_by_address WHERE 
                    tx=?1 AND
                    address=?2
                ",
                params![tx_rowid, address_rowid],
                |row| row.get(0),
            )
            .unwrap();

        assert!(tx_by_address > 0);
    }

    #[test]
    fn test_add_tx() {
        let mut conn = Connection::open(":memory:").unwrap();
        prepare_schema(&conn).unwrap();

        let mut utxos = BTreeMap::new();

        let addr_str = "Ae2tdPwUPEZKmwoy3AU3cXb5Chnasj6mvVNxV1H11997q3VW5ihbSfQwGpm";
        let bytes = base58::decode(addr_str).unwrap();
        let address = ExtendedAddr::try_from_slice(&bytes).unwrap();
        let id = hash::Blake2b256::new(&[0]);

        let value = 10000;

        utxos.insert(
            TxoPointer { id, index: 0 },
            TxOut {
                address: address.clone(),
                value: Coin::new(value).unwrap(),
            },
        );

        let initial = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91842",
        )
        .unwrap();
        apply_initial_state(&mut conn, &utxos).unwrap();

        let mut tx = Tx::new();

        let input = TxoPointer { id, index: 0 };

        let addr_dest_str = "DdzFFzCqrhsyhumccfGyEj3WZzztSPr92ntRWB6UVVwzcMTpwoafVQ5vD9mdZ5Xind8ycugbmA8esxmo7NycjQFGSbDeKrxabTz8MVzf";
        let address_dest =
            ExtendedAddr::try_from_slice(&base58::decode(addr_dest_str).unwrap()).unwrap();

        let output = TxOut {
            address: address_dest,
            value: Coin::new(5000).unwrap(),
        };

        let rest = TxOut {
            address: address.clone(),
            value: Coin::new(5000).unwrap(),
        };

        tx.add_input(input);
        tx.add_output(output);
        tx.add_output(rest);

        insert_tx(&mut conn, tx.clone()).unwrap();

        let tx_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM tx WHERE txid=?1",
                params![format!("{}", tx.id())],
                |row| row.get(0),
            )
            .unwrap();

        assert!(tx_rowid == 2);

        let output1_rowid: i64 = conn
            .query_row(
                "SELECT output.id FROM output join address WHERE 
                    tx=?1 AND
                    address.address=?2 AND
                    offset=?3 AND
                    value=?4
                ",
                params![tx_rowid, addr_dest_str, 0, 5000i64],
                |row| row.get(0),
            )
            .unwrap();

        assert!(output1_rowid > 0);

        let output2_rowid: i64 = conn
            .query_row(
                "SELECT output.id FROM output join address WHERE 
                    tx=?1 AND
                    address.address=?2 AND
                    offset=?3 AND
                    value=?4
                ",
                params![tx_rowid, addr_str, 1, 5000i64],
                |row| row.get(0),
            )
            .unwrap();

        assert!(output2_rowid > 0);

        let input_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM input WHERE 
                    tx=?1 AND
                    offset=?2
                ",
                params![tx_rowid, 0],
                |row| row.get(0),
            )
            .unwrap();

        assert!(input_rowid > 0);

        let tx_by_address_1: i64 = conn
            .query_row(
                "SELECT txs_by_address.id FROM 
                    txs_by_address join address
                    on txs_by_address.address = address.id
                WHERE 
                    tx=?1 AND
                    address.address=?2
                ",
                params![tx_rowid, addr_str],
                |row| row.get(0),
            )
            .unwrap();

        assert!(tx_by_address_1 > 0);

        let tx_by_address_2: i64 = conn
            .query_row(
                "SELECT txs_by_address.id FROM 
                    txs_by_address join address
                    on txs_by_address.address = address.id
                WHERE 
                    tx=?1 AND
                    address.address=?2
                ",
                params![tx_rowid, addr_dest_str],
                |row| row.get(0),
            )
            .unwrap();

        assert!(tx_by_address_2 > 0);
    }

    #[test]
    fn test_transactions_by_address() {
        let mut conn = Connection::open(":memory:").unwrap();
        prepare_schema(&conn).unwrap();

        let mut utxos = BTreeMap::new();

        let addr_str = "Ae2tdPwUPEZKmwoy3AU3cXb5Chnasj6mvVNxV1H11997q3VW5ihbSfQwGpm";
        let bytes = base58::decode(addr_str).unwrap();
        let address = ExtendedAddr::try_from_slice(&bytes).unwrap();
        let id = hash::Blake2b256::new(&[0]);

        let value = 10000;

        utxos.insert(
            TxoPointer { id, index: 0 },
            TxOut {
                address: address.clone(),
                value: Coin::new(value).unwrap(),
            },
        );

        let initial = HeaderHash::from_str(
            "ae443ffffe52cc29de83312d2819b3955fc306ce65ae6aa5b26f1d3c76e91842",
        )
        .unwrap();
        apply_initial_state(&mut conn, &utxos).unwrap();

        let mut tx = Tx::new();

        let input = TxoPointer { id, index: 0 };

        let addr_dest_str = "DdzFFzCqrhsyhumccfGyEj3WZzztSPr92ntRWB6UVVwzcMTpwoafVQ5vD9mdZ5Xind8ycugbmA8esxmo7NycjQFGSbDeKrxabTz8MVzf";
        let address_dest =
            ExtendedAddr::try_from_slice(&base58::decode(addr_dest_str).unwrap()).unwrap();

        let output = TxOut {
            address: address_dest.clone(),
            value: Coin::new(5000).unwrap(),
        };

        let rest = TxOut {
            address: address.clone(),
            value: Coin::new(5000).unwrap(),
        };

        tx.add_input(input);
        tx.add_output(output);
        tx.add_output(rest);

        insert_tx(&mut conn, tx.clone()).unwrap();

        let transactions = transactions_of(&conn, address.clone()).unwrap();

        let transaction1 = Transaction {
            txid: format!("{}", id),
            inputs: vec![],
            outputs: vec![Output {
                value: 10000,
                address: format!("{}", address.clone()),
            }],
        };

        assert!(transactions
            .iter()
            .any(|transaction| { transaction == &transaction1 }));

        let transaction2 = Transaction {
            txid: format!("{}", tx.id()),
            inputs: vec![Input {
                id: format!("{}", id),
                index: 0,
            }],
            outputs: vec![
                Output {
                    address: format!("{}", address_dest.clone()),
                    value: 5000,
                },
                Output {
                    address: format!("{}", address.clone()),
                    value: 5000,
                },
            ],
        };

        assert!(transactions
            .iter()
            .any(|transaction| { transaction == &transaction2 }));
    }
}
