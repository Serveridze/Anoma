//! By default, these tests will run in release mode. This can be disabled
//! by setting environment variable `ANOMA_E2E_DEBUG=true`. For debugging,
//! you'll typically also want to set `RUST_BACKTRACE=1`, e.g.:
//!
//! ```ignore,shell
//! ANOMA_E2E_DEBUG=true RUST_BACKTRACE=1 cargo test e2e::ibc_tests -- --test-threads=1 --nocapture
//! ```
//!
//! To keep the temporary files created by a test, use env var
//! `ANOMA_E2E_KEEP_TEMP=true`.

use core::convert::TryFrom;
use core::str::FromStr;
use core::time::Duration;

use anoma::ibc::applications::ics20_fungible_token_transfer::msgs::transfer::MsgTransfer;
use anoma::ibc::clients::ics07_tendermint::client_state::{
    AllowUpdate, ClientState as TmClientState,
};
use anoma::ibc::clients::ics07_tendermint::consensus_state::ConsensusState as TmConsensusState;
use anoma::ibc::core::ics02_client::client_consensus::{
    AnyConsensusState, ConsensusState,
};
use anoma::ibc::core::ics02_client::client_state::{
    AnyClientState, ClientState,
};
use anoma::ibc::core::ics02_client::header::Header;
use anoma::ibc::core::ics02_client::height::Height;
use anoma::ibc::core::ics02_client::msgs::create_client::MsgCreateAnyClient;
use anoma::ibc::core::ics02_client::msgs::update_client::MsgUpdateAnyClient;
use anoma::ibc::core::ics02_client::trust_threshold::TrustThreshold;
use anoma::ibc::core::ics03_connection::connection::Counterparty as ConnCounterparty;
use anoma::ibc::core::ics03_connection::msgs::conn_open_ack::MsgConnectionOpenAck;
use anoma::ibc::core::ics03_connection::msgs::conn_open_confirm::MsgConnectionOpenConfirm;
use anoma::ibc::core::ics03_connection::msgs::conn_open_init::MsgConnectionOpenInit;
use anoma::ibc::core::ics03_connection::msgs::conn_open_try::MsgConnectionOpenTry;
use anoma::ibc::core::ics03_connection::version::Version as ConnVersion;
use anoma::ibc::core::ics04_channel::channel::{
    ChannelEnd, Counterparty as ChanCounterparty, Order as ChanOrder,
    State as ChanState,
};
use anoma::ibc::core::ics04_channel::msgs::acknowledgement::MsgAcknowledgement;
use anoma::ibc::core::ics04_channel::msgs::chan_open_ack::MsgChannelOpenAck;
use anoma::ibc::core::ics04_channel::msgs::chan_open_confirm::MsgChannelOpenConfirm;
use anoma::ibc::core::ics04_channel::msgs::chan_open_init::MsgChannelOpenInit;
use anoma::ibc::core::ics04_channel::msgs::chan_open_try::MsgChannelOpenTry;
use anoma::ibc::core::ics04_channel::msgs::recv_packet::MsgRecvPacket;
use anoma::ibc::core::ics04_channel::packet::Packet;
use anoma::ibc::core::ics04_channel::Version as ChanVersion;
use anoma::ibc::core::ics23_commitment::commitment::CommitmentProofBytes;
use anoma::ibc::core::ics23_commitment::merkle::convert_tm_to_ics_merkle_proof;
use anoma::ibc::core::ics23_commitment::specs::ProofSpecs;
use anoma::ibc::core::ics24_host::identifier::{
    ChainId, ClientId, ConnectionId, PortChannelId, PortId,
};
use anoma::ibc::events::{from_tx_response_event, IbcEvent};
use anoma::ibc::proofs::Proofs;
use anoma::ibc::signer::Signer;
use anoma::ibc::timestamp::Timestamp;
use anoma::ibc::tx_msg::Msg;
use anoma::ibc_proto::cosmos::base::v1beta1::Coin;
use anoma::ledger::ibc::handler::{commitment_prefix, port_channel_id};
use anoma::ledger::ibc::storage::*;
use anoma::ledger::storage::{MerkleTree, Sha256Hasher};
use anoma::tendermint::block::Header as TmHeader;
use anoma::tendermint::merkle::proof::Proof as TmProof;
use anoma::tendermint::node::Id as TendermintNodeId;
use anoma::tendermint::trust_threshold::TrustThresholdFraction;
use anoma::tendermint_proto::Protobuf;
use anoma::types::storage::Key;
use anoma_apps::client::rpc::query_storage_value_bytes;
use anoma_apps::client::utils::id_from_pk;
use color_eyre::eyre::Result;
use eyre::eyre;
#[cfg(not(feature = "ABCI"))]
use ibc_relayer::config::types::{MaxMsgNum, MaxTxSize, Memo};
#[cfg(not(feature = "ABCI"))]
use ibc_relayer::config::ChainConfig;
#[cfg(not(feature = "ABCI"))]
use ibc_relayer::config::{AddressType, GasPrice, PacketFilter};
#[cfg(not(feature = "ABCI"))]
use ibc_relayer::keyring::Store;
#[cfg(not(feature = "ABCI"))]
use ibc_relayer::light_client::tendermint::LightClient as TmLightClient;
#[cfg(not(feature = "ABCI"))]
use ibc_relayer::light_client::{LightClient, Verified};
#[cfg(feature = "ABCI")]
use ibc_relayer_abci::config::types::{MaxMsgNum, MaxTxSize, Memo};
#[cfg(feature = "ABCI")]
use ibc_relayer_abci::config::ChainConfig;
#[cfg(feature = "ABCI")]
use ibc_relayer_abci::config::{AddressType, GasPrice, PacketFilter};
#[cfg(feature = "ABCI")]
use ibc_relayer_abci::keyring::Store;
#[cfg(feature = "ABCI")]
use ibc_relayer_abci::light_client::tendermint::LightClient as TmLightClient;
#[cfg(feature = "ABCI")]
use ibc_relayer_abci::light_client::{LightClient, Verified};
use setup::constants::*;
#[cfg(not(feature = "ABCI"))]
use tendermint_config::net::Address as TendermintAddress;
#[cfg(feature = "ABCI")]
use tendermint_config_abci::net::Address as TendermintAddress;
#[cfg(not(feature = "ABCI"))]
use tendermint_rpc::{Client, HttpClient, Url};
#[cfg(feature = "ABCI")]
use tendermint_rpc_abci::{Client, HttpClient, Url};
use tokio::runtime::Runtime;

use crate::e2e::helpers::{find_address, get_actor_rpc, get_epoch};
use crate::e2e::setup::{self, sleep, Bin, Test, Who};
use crate::{run, run_as};

#[test]
fn run_ledger_ibc() -> Result<()> {
    let (test_a, test_b) = setup::two_single_node_nets()?;

    // Run Chain A
    let mut ledger_a =
        run_as!(test_a, Who::Validator(0), Bin::Node, &["ledger"], Some(40))?;
    ledger_a.exp_string("Anoma ledger node started")?;
    // Run Chain B
    let mut ledger_b =
        run_as!(test_b, Who::Validator(0), Bin::Node, &["ledger"], Some(40))?;
    ledger_b.exp_string("Anoma ledger node started")?;

    sleep(5);

    let (client_id_a, client_id_b) = create_client(&test_a, &test_b)?;

    let (conn_id_a, conn_id_b) =
        connection_handshake(&test_a, &test_b, &client_id_a, &client_id_b)?;

    let (port_channel_id_a, _) =
        channel_handshake(&test_a, &test_b, &conn_id_a, &conn_id_b)?;

    transfer_token(&test_a, &test_b, &port_channel_id_a)?;

    // Check the balance on Chain A
    let rpc_a = get_actor_rpc(&test_a, &Who::Validator(0));
    let query_args = vec![
        "balance",
        "--owner",
        ALBERT,
        "--token",
        XAN,
        "--ledger-address",
        &rpc_a,
    ];
    let expected = r"XAN: 0";
    let mut client = run!(test_a, Bin::Client, query_args, Some(40))?;
    client.exp_regex(expected)?;
    client.assert_success();

    // Check the balance on Chain B
    let rpc_b = get_actor_rpc(&test_b, &Who::Validator(0));
    let query_args = vec![
        "balance",
        "--owner",
        BERTHA,
        "--token",
        XAN,
        "--ledger-address",
        &rpc_b,
    ];
    let expected = r"XAN: 2000000";
    let mut client = run!(test_b, Bin::Client, query_args, Some(40))?;
    client.exp_regex(expected)?;
    client.assert_success();

    Ok(())
}

fn create_client(test_a: &Test, test_b: &Test) -> Result<(ClientId, ClientId)> {
    let height = query_height(test_b)?;
    let client_state = make_client_state(test_b, height);
    let height = client_state.latest_height();
    let message = MsgCreateAnyClient {
        client_state,
        consensus_state: make_consensus_state(test_b, height)?,
        signer: Signer::new("test_a"),
    };
    let height_a = submit_ibc_tx(test_a, message)?;

    let height = query_height(test_a)?;
    let client_state = make_client_state(test_a, height);
    let height = client_state.latest_height();
    let message = MsgCreateAnyClient {
        client_state,
        consensus_state: make_consensus_state(test_a, height)?,
        signer: Signer::new("test_b"),
    };
    let height_b = submit_ibc_tx(test_b, message)?;

    let client_id_a = match get_event(test_a, height_a)? {
        Some(IbcEvent::CreateClient(event)) => event.client_id().clone(),
        _ => return Err(eyre!("Transaction failed")),
    };
    let client_id_b = match get_event(test_b, height_b)? {
        Some(IbcEvent::CreateClient(event)) => event.client_id().clone(),
        _ => return Err(eyre!("Transaction failed")),
    };

    // `client_id_a` represents the ID of the B's client on Chain A
    Ok((client_id_a, client_id_b))
}

fn make_client_state(test: &Test, height: Height) -> AnyClientState {
    let unbonding_period = Duration::new(1814400, 0);
    let trusting_period = 2 * unbonding_period / 3;
    let max_clock_drift = Duration::new(60, 0);
    let chain_id = ChainId::from_str(test.net.chain_id.as_str()).unwrap();
    TmClientState::new(
        chain_id,
        TrustThreshold::default(),
        trusting_period,
        unbonding_period,
        max_clock_drift,
        height,
        MerkleTree::<Sha256Hasher>::default().proof_specs().into(),
        vec!["upgrade".to_string(), "upgradedIBCState".to_string()],
        AllowUpdate {
            after_expiry: true,
            after_misbehaviour: true,
        },
    )
    .unwrap()
    .wrap_any()
}

fn make_consensus_state(
    test: &Test,
    height: Height,
) -> Result<AnyConsensusState> {
    let header = query_header(test, height)?;
    Ok(TmConsensusState::from(header).wrap_any())
}

fn proof_specs() -> ProofSpecs {
    MerkleTree::<Sha256Hasher>::default().proof_specs().into()
}

fn update_to_latest_client(
    src_test: &Test,
    target_test: &Test,
    client_id: &ClientId,
) -> Result<()> {
    // check the current state on the target chain
    let client_state = query_client_state(target_test, client_id)?;
    let trusted_height = client_state.latest_height();
    // get the latest height on the source chain
    let target_height = query_height(src_test)?;
    update_client(
        target_test,
        client_id,
        trusted_height,
        target_height,
        &client_state,
    )
}

fn update_client(
    test: &Test,
    client_id: &ClientId,
    trusted_height: Height,
    target_height: Height,
    client_state: &AnyClientState,
) -> Result<()> {
    let config = dummy_chain_config(test);
    let pk = test
        .genesis
        .validator
        .get("validator-0")
        .unwrap()
        .account_public_key
        .as_ref()
        .unwrap();
    let peer_id: TendermintNodeId = id_from_pk(pk.to_public_key().unwrap());
    let mut light_client =
        TmLightClient::from_config(&config, peer_id).unwrap();
    let Verified { target, supporting } = light_client
        .header_and_minimal_set(trusted_height, target_height, client_state)
        .map_err(|e| eyre!("Building the header failed: {}", e))?;

    for header in supporting {
        let message = MsgUpdateAnyClient {
            header: header.wrap_any(),
            client_id: client_id.clone(),
            signer: Signer::new("test"),
        };
        submit_ibc_tx(test, message)?;
    }

    let message = MsgUpdateAnyClient {
        header: target.wrap_any(),
        client_id: client_id.clone(),
        signer: Signer::new("test"),
    };
    submit_ibc_tx(test, message)?;

    Ok(())
}

fn dummy_chain_config(test: &Test) -> ChainConfig {
    let rpc_addr =
        Url::from_str(&get_actor_rpc(test, &Who::Validator(0))).unwrap();
    // use only id and rpc_addr
    ChainConfig {
        id: ChainId::new(test.net.chain_id.as_str().to_string(), 0),
        rpc_addr: rpc_addr.clone(),
        websocket_addr: rpc_addr.clone(),
        grpc_addr: rpc_addr,
        rpc_timeout: Duration::new(10, 0),
        account_prefix: "dummy".to_string(),
        key_name: "dummy".to_string(),
        key_store_type: Store::default(),
        store_prefix: "dummy".to_string(),
        default_gas: None,
        max_gas: None,
        gas_adjustment: None,
        fee_granter: None,
        max_msg_num: MaxMsgNum::default(),
        max_tx_size: MaxTxSize::default(),
        clock_drift: Duration::new(5, 0),
        max_block_time: Duration::new(5, 0),
        trusting_period: None,
        memo_prefix: Memo::default(),
        proof_specs: proof_specs(),
        trust_threshold: TrustThresholdFraction::ONE_THIRD,
        gas_price: GasPrice::new(0.0, "dummy".to_string()),
        packet_filter: PacketFilter::default(),
        address_type: AddressType::Cosmos,
    }
}

fn connection_handshake(
    test_a: &Test,
    test_b: &Test,
    client_id_a: &ClientId,
    client_id_b: &ClientId,
) -> Result<(ConnectionId, ConnectionId)> {
    // OpenInitConnection on Chain A
    let msg = MsgConnectionOpenInit {
        client_id: client_id_a.clone(),
        counterparty: ConnCounterparty::new(
            client_id_b.clone(),
            None,
            commitment_prefix(),
        ),
        version: Some(ConnVersion::default()),
        delay_period: Duration::new(30, 0),
        signer: Signer::new("test_a"),
    };
    let height = submit_ibc_tx(test_a, msg)?;
    let conn_id_a = match get_event(test_a, height)? {
        Some(IbcEvent::OpenInitConnection(event)) => event
            .connection_id()
            .clone()
            .ok_or(eyre!("No connection ID is set"))?,
        _ => return Err(eyre!("Transaction failed")),
    };

    // Update the client state of Chain A on Chain B
    update_to_latest_client(test_a, test_b, client_id_b)?;

    // OpenTryConnection on Chain B
    // get the B's proofs on Chain A
    let proofs = get_connection_proofs(test_a, &conn_id_a)?;
    let counterparty = ConnCounterparty::new(
        client_id_a.clone(),
        Some(conn_id_a.clone()),
        commitment_prefix(),
    );
    let msg = MsgConnectionOpenTry {
        previous_connection_id: None,
        client_id: client_id_b.clone(),
        client_state: None,
        counterparty,
        counterparty_versions: vec![ConnVersion::default()],
        proofs,
        delay_period: Duration::new(30, 0),
        signer: Signer::new("test_b"),
    };
    let height = submit_ibc_tx(test_b, msg)?;
    let conn_id_b = match get_event(test_b, height)? {
        Some(IbcEvent::OpenTryConnection(event)) => event
            .connection_id()
            .clone()
            .ok_or(eyre!("No connection ID is set"))?,
        _ => return Err(eyre!("Transaction failed")),
    };

    // OpenAckConnection on Chain A
    // get the A's proofs on Chain B
    let proofs = get_connection_proofs(test_b, &conn_id_b)?;
    let msg = MsgConnectionOpenAck {
        connection_id: conn_id_a.clone(),
        counterparty_connection_id: conn_id_b.clone(),
        client_state: None,
        proofs,
        version: ConnVersion::default(),
        signer: Signer::new("test_a"),
    };
    submit_ibc_tx(test_a, msg)?;

    // OpenConfirmConnection on Chain B
    // get the proofs on Chain A
    let proofs = get_connection_proofs(test_a, &conn_id_a)?;
    let msg = MsgConnectionOpenConfirm {
        connection_id: conn_id_b.clone(),
        proofs,
        signer: Signer::new("test_b"),
    };
    submit_ibc_tx(test_b, msg)?;

    Ok((conn_id_a, conn_id_b))
}

fn get_connection_proofs(
    test: &Test,
    conn_id: &ConnectionId,
) -> Result<Proofs> {
    let height = query_height(test)?;
    let key = connection_key(&conn_id);
    let tm_proof = query_proof(test, &key)?;
    let connection_proof = convert_proof(tm_proof)?;

    Proofs::new(connection_proof, None, None, None, height)
        .map_err(|e| eyre!("Creating proofs failed: error {}", e))
}

fn channel_handshake(
    test_a: &Test,
    test_b: &Test,
    conn_id_a: &ConnectionId,
    conn_id_b: &ConnectionId,
) -> Result<(PortChannelId, PortChannelId)> {
    // OpenInitChannel on Chain A
    let port_id = PortId::from_str("test_port").unwrap();
    let counterparty = ChanCounterparty::new(port_id.clone(), None);
    let channel = ChannelEnd::new(
        ChanState::Uninitialized,
        ChanOrder::Unordered,
        counterparty,
        vec![conn_id_a.clone()],
        ChanVersion::ics20(),
    );
    let msg = MsgChannelOpenInit {
        port_id: port_id.clone(),
        channel,
        signer: Signer::new("test_a"),
    };
    let height = submit_ibc_tx(test_a, msg)?;
    let channel_id_a = match get_event(test_a, height)? {
        Some(IbcEvent::OpenInitChannel(event)) => event
            .channel_id()
            .ok_or(eyre!("No channel ID is set"))?
            .clone(),
        _ => return Err(eyre!("Transaction failed")),
    };
    let port_channel_id_a =
        port_channel_id(port_id.clone(), channel_id_a.clone());

    // OpenTryChannel on Chain B
    let counterparty =
        ChanCounterparty::new(port_id.clone(), Some(channel_id_a.clone()));
    let channel = ChannelEnd::new(
        ChanState::Uninitialized,
        ChanOrder::Unordered,
        counterparty,
        vec![conn_id_b.clone()],
        ChanVersion::ics20(),
    );
    let proofs = get_channel_proofs(test_a, &port_channel_id_a)?;
    let msg = MsgChannelOpenTry {
        port_id: port_id.clone(),
        previous_channel_id: None,
        channel,
        counterparty_version: ChanVersion::ics20(),
        proofs,
        signer: Signer::new("test_b"),
    };
    let height = submit_ibc_tx(test_b, msg)?;
    let channel_id_b = match get_event(test_b, height)? {
        Some(IbcEvent::OpenInitChannel(event)) => event
            .channel_id()
            .ok_or(eyre!("No channel ID is set"))?
            .clone(),
        _ => return Err(eyre!("Transaction failed")),
    };
    let port_channel_id_b =
        port_channel_id(port_id.clone(), channel_id_b.clone());

    // OpenAckChannel on Chain A
    let proofs = get_channel_proofs(test_b, &port_channel_id_b)?;
    let msg = MsgChannelOpenAck {
        port_id: port_id.clone(),
        channel_id: channel_id_a,
        counterparty_channel_id: channel_id_b.clone(),
        counterparty_version: ChanVersion::ics20(),
        proofs,
        signer: Signer::new("test_a"),
    };
    submit_ibc_tx(test_a, msg)?;

    // OpenConfirmChannel on Chain B
    let proofs = get_channel_proofs(test_a, &port_channel_id_a)?;
    let msg = MsgChannelOpenConfirm {
        port_id,
        channel_id: channel_id_b,
        proofs,
        signer: Signer::new("test_b"),
    };
    submit_ibc_tx(test_b, msg)?;

    Ok((port_channel_id_a, port_channel_id_b))
}

fn get_channel_proofs(
    test: &Test,
    port_channel_id: &PortChannelId,
) -> Result<Proofs> {
    let height = query_height(test)?;
    let key = channel_key(port_channel_id);
    let tm_proof = query_proof(test, &key)?;
    let proof = convert_proof(tm_proof)?;

    Proofs::new(proof, None, None, None, height)
        .map_err(|e| eyre!("Creating proofs failed: error {}", e))
}

fn transfer_token(
    test_a: &Test,
    test_b: &Test,
    source_port_channel_id: &PortChannelId,
) -> Result<()> {
    let xan = find_address(test_a, XAN)?;
    let sender = find_address(test_a, ALBERT)?;
    let receiver = find_address(test_b, BERTHA)?;

    // Send a token from Chain A
    let token = Some(Coin {
        denom: xan.to_string(),
        amount: "1000000".to_string(),
    });
    let msg = MsgTransfer {
        source_port: source_port_channel_id.port_id.clone(),
        source_channel: source_port_channel_id.channel_id.clone(),
        token,
        sender: Signer::new(sender.to_string()),
        receiver: Signer::new(receiver.to_string()),
        timeout_height: Height::new(100, 100),
        timeout_timestamp: (Timestamp::now() + Duration::new(30, 0)).unwrap(),
    };
    let height = submit_ibc_tx(test_a, msg)?;
    let packet = match get_event(test_a, height)? {
        Some(IbcEvent::SendPacket(event)) => event.packet,
        _ => return Err(eyre!("Transaction failed")),
    };

    // Receive the token on Chain B
    let proofs = get_commitment_proof(test_a, &packet)?;
    let msg = MsgRecvPacket {
        packet,
        proofs,
        signer: Signer::new("test_b"),
    };
    let height = submit_ibc_tx(test_b, msg)?;
    let (acknowledgement, packet) = match get_event(test_b, height)? {
        Some(IbcEvent::WriteAcknowledgement(event)) => {
            (event.ack, event.packet)
        }
        _ => return Err(eyre!("Transaction failed")),
    };

    // Acknowledge on Chain A
    let proofs = get_ack_proof(test_a, &packet)?;
    let msg = MsgAcknowledgement {
        packet,
        acknowledgement: acknowledgement.into(),
        proofs,
        signer: Signer::new("test_a"),
    };
    submit_ibc_tx(test_b, msg)?;

    Ok(())
}

fn get_commitment_proof(test: &Test, packet: &Packet) -> Result<Proofs> {
    let height = query_height(test)?;
    let key = commitment_key(
        &packet.source_port,
        &packet.source_channel,
        packet.sequence,
    );
    let tm_proof = query_proof(test, &key)?;
    let commitment_proof = convert_proof(tm_proof)?;

    Proofs::new(commitment_proof, None, None, None, height)
        .map_err(|e| eyre!("Creating proofs failed: error {}", e))
}

fn get_ack_proof(test: &Test, packet: &Packet) -> Result<Proofs> {
    let height = query_height(test)?;
    let key = ack_key(
        &packet.destination_port,
        &packet.destination_channel,
        packet.sequence,
    );
    let tm_proof = query_proof(test, &key)?;
    let ack_proof = convert_proof(tm_proof)?;

    Proofs::new(ack_proof, None, None, None, height)
        .map_err(|e| eyre!("Creating proofs failed: error {}", e))
}

fn submit_ibc_tx(
    test: &Test,
    message: impl Msg + std::fmt::Debug,
) -> Result<u32> {
    let data_path = test.base_dir.path().join("tx.data");
    let data = make_ibc_data(message.clone());
    std::fs::write(&data_path, data).expect("writing data failed");

    let code_path = wasm_abs_path(TX_IBC_WASM);
    let code_path = code_path.to_string_lossy();
    let data_path = data_path.to_string_lossy();
    let rpc = get_actor_rpc(test, &Who::Validator(0));
    let mut client = run!(
        test,
        Bin::Client,
        [
            "tx",
            "--code-path",
            &code_path,
            "--data-path",
            &data_path,
            "--signer",
            ALBERT,
            "--fee-amount",
            "0",
            "--gas-limit",
            "0",
            "--fee-token",
            XAN,
            "--ledger-address",
            &rpc
        ],
        Some(40)
    )?;
    let (unread, matched) = client.exp_regex("\"height\": .*,")?;
    let height_str = matched
        .trim()
        .rsplit_once(' ')
        .unwrap()
        .1
        .replace('"', "")
        .replace(',', "");
    let height = height_str.parse().unwrap();

    let (_unread, matched) = client.exp_regex("\"code\": .*,")?;
    let code = matched
        .trim()
        .rsplit_once(' ')
        .unwrap()
        .1
        .replace('"', "")
        .replace(',', "");
    if code != "0" {
        return Err(eyre!(
            "The transaction failed: message {:?}, unread {}",
            message,
            unread
        ));
    }

    Ok(height)
}

fn make_ibc_data(message: impl Msg) -> Vec<u8> {
    let msg = message.to_any();
    let mut tx_data = vec![];
    prost::Message::encode(&msg, &mut tx_data)
        .expect("encoding IBC message shouldn't fail");
    tx_data
}

fn query_height(test: &Test) -> Result<Height> {
    let rpc = get_actor_rpc(test, &Who::Validator(0));
    let ledger_address = TendermintAddress::from_str(&rpc).unwrap();
    let client = HttpClient::new(ledger_address).unwrap();
    let rt = Runtime::new().unwrap();

    let status = rt
        .block_on(client.status())
        .map_err(|e| eyre!("Getting the status failed: {}", e))?;
    let epoch = get_epoch(test, &rpc)?;

    Ok(Height::new(
        epoch.0,
        status.sync_info.latest_block_height.into(),
    ))
}

fn query_header(test: &Test, height: Height) -> Result<TmHeader> {
    let rpc = get_actor_rpc(test, &Who::Validator(0));
    let ledger_address = TendermintAddress::from_str(&rpc).unwrap();
    let client = HttpClient::new(ledger_address).unwrap();
    let height = height.revision_height as u32;
    let result = Runtime::new()
        .unwrap()
        .block_on(client.blockchain(height, height));
    match result {
        Ok(mut response) => match response.block_metas.pop() {
            Some(meta) => Ok(meta.header),
            None => Err(eyre!("No meta exists")),
        },
        Err(e) => Err(eyre!("Header query failed: {}", e)),
    }
}

fn get_event(test: &Test, height: u32) -> Result<Option<IbcEvent>> {
    let rpc = get_actor_rpc(test, &Who::Validator(0));
    let ledger_address = TendermintAddress::from_str(&rpc).unwrap();
    let client = HttpClient::new(ledger_address).unwrap();

    let response = Runtime::new()
        .unwrap()
        .block_on(client.block_results(height))
        .map_err(|e| eyre!("block_results() for an IBC event failed: {}", e))?;
    let tx_results = response.txs_results.ok_or_else(|| {
        eyre!("No transaction has been executed: height {}", height)
    })?;
    for result in tx_results {
        if result.code.is_err() {
            return Err(eyre!(
                "The transaction failed: code {:?}, log {}",
                result.code,
                result.log
            ));
        }
    }
    let events = response
        .end_block_events
        .ok_or_else(|| eyre!("IBC event was not found: height {}", height))?;
    for event in &events {
        // The height will be set, but not be used
        let dummy_height = Height::new(0, 0);
        match from_tx_response_event(dummy_height, event) {
            Some(ibc_event) => return Ok(Some(ibc_event)),
            None => continue,
        }
    }
    // No IBC event was found
    Ok(None)
}

fn query_client_state(
    test: &Test,
    client_id: &ClientId,
) -> Result<AnyClientState> {
    let rpc = get_actor_rpc(test, &Who::Validator(0));
    let ledger_address = TendermintAddress::from_str(&rpc).unwrap();
    let client = HttpClient::new(ledger_address).unwrap();
    let key = client_state_key(client_id);
    let result = Runtime::new()
        .unwrap()
        .block_on(query_storage_value_bytes(client, key, false));
    match result {
        (Some(value), _) => AnyClientState::decode_vec(&value)
            .map_err(|e| eyre!("Decoding the client state failed: {}", e)),
        _ => Err(eyre!("Getting the client state failed")),
    }
}

fn query_with_proof(test: &Test, key: &Key) -> Result<(Vec<u8>, TmProof)> {
    let rpc = get_actor_rpc(test, &Who::Validator(0));
    let ledger_address = TendermintAddress::from_str(&rpc).unwrap();
    let client = HttpClient::new(ledger_address).unwrap();
    let result = Runtime::new()
        .unwrap()
        .block_on(query_storage_value_bytes(&client, key, true));
    match result {
        (Some(value), Some(proof)) => Ok((value, proof)),
        _ => Err(eyre!("The value doesn't exist: key {}", key)),
    }
}

fn query_proof(test: &Test, key: &Key) -> Result<TmProof> {
    let rpc = get_actor_rpc(test, &Who::Validator(0));
    let ledger_address = TendermintAddress::from_str(&rpc).unwrap();
    let client = HttpClient::new(ledger_address).unwrap();
    let result = Runtime::new()
        .unwrap()
        .block_on(query_storage_value_bytes(&client, key, true));
    match result {
        (_, Some(proof)) => Ok(proof),
        _ => Err(eyre!("Proof doesn't exist: key {}", key)),
    }
}

fn convert_proof(tm_proof: TmProof) -> Result<CommitmentProofBytes> {
    let merkle_proof = convert_tm_to_ics_merkle_proof(&tm_proof)
        .map_err(|e| eyre!("Proof conversion to MerkleProof failed: {}", e))?;
    CommitmentProofBytes::try_from(merkle_proof).map_err(|e| {
        eyre!("Proof conversion to CommitmentProofBytes failed: {}", e)
    })
}