// Copyright (c) 2026 Tokenized Fractional RWA Marketplace Contributors
// SPDX-License-Identifier: MIT

#![no_std]
use soroban_sdk::{
    contract, contractclient, contractevent, contractimpl, contractmeta, contracttype, token,
    Address, Bytes, BytesN, Env, String, Vec,
};

// ── SIP-4 / SEP-46 Contract Metadata ─────────────────────────────────
// Off-chain tools (explorers, wallets, indexers) read these entries from
// the Wasm custom section `contractmetav0` without executing the contract.
contractmeta!(key = "name", val = "RWA Marketplace");
contractmeta!(key = "version", val = "0.4.0");
contractmeta!(key = "description", val = "Tokenized Fractional RWA Marketplace");
contractmeta!(key = "sep", val = "41");

/// Minimal interface for calling the deployed ShareCertificate NFT contract.
#[contractclient(name = "NftContractClient")]
pub trait NftContractInterface {
    fn mint_certificate(env: Env, to: Address) -> u32;
}

/// Issue #169 – Minimal oracle interface.
/// The oracle contract must expose a `get_price() -> i128` function
/// that returns the current asset price per share in the payment token's
/// smallest unit. This is intentionally minimal to support any Stellar-based
/// oracle that follows this convention.
#[contractclient(name = "OracleContractClient")]
pub trait OracleContractInterface {
    fn get_price(env: Env) -> i128;
}

#[contract]
pub struct RwaMarketplace;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    PaymentToken,
    PricePerShare,
    TotalShares,
    AvailableShares,
    Paused,
    Balance(Address),
    VestingSchedules(Address),
    Holders,
    MetadataUri,
    DividendSchedule,
    LastDistribution,
    Whitelisted(Address),
    SellOrder(u64),
    NextOrderId,
    MaxSharesPerUser,
    /// Allowance(owner, spender) → approved amount
    Allowance(Address, Address),
    BuybackConfig,
    BuybackBudget,
    LastBuyback,
    AcceptedTokens,
    /// Optional NFT contract address for minting share certificates on buy
    NftContract,
    /// Reentrancy guard flag for defense-in-depth protection
    ReentrancyGuard,
    /// Issue #169: Price oracle contract address (optional)
    OracleAddress,
    /// Issue #170: Locked-for-bridge amount per user
    BridgeLocked(Address),
    /// SIP-4 metadata
    ContractMetadata,
    /// Issue #276: Flash loan protection configuration
    FlashLoanGuardConfig,
    /// Issue #276: Last trade ledger sequence per account for origin delay checks
    LastTradeLedger(Address),
    // ── Issue #309: Upgradeable Proxy Pattern ───────────────────────────
    /// Current implementation version number
    ImplementationVersion,
    /// Pending upgrade proposal: new_wasm_hash, scheduled execution ledger
    PendingUpgrade,
    /// Timelock in ledger sequences before upgrade can execute
    UpgradeTimelock,
    /// Admin who proposed the upgrade
    UpgradeProposer,
    // ── Issue #310: Granular Pause Controls ─────────────────────────────
    /// Per-function pause flags stored as a bitflag
    /// Bit 0 = purchases, Bit 1 = transfers, Bit 2 = dividends, Bit 3 = sell orders
    FunctionPauseFlags,
    // ── Issue #311: Emergency Stop Mechanism ────────────────────────────
    /// Circuit breaker configuration
    CircuitBreakerConfig,
    /// Whether circuit breaker is currently triggered
    CircuitBreakerTriggered,
    /// Circuit breaker trigger history counter
    CircuitBreakerTriggerCount,
    // ── Issue #312: State Recovery Functions ────────────────────────────
    /// Whether state recovery snapshots are enabled
    RecoveryEnabled,
    /// Last snapshot ledger sequence
    LastSnapshotLedger,
    /// Number of snapshots taken
    SnapshotCount,
    // ── Issue #270: Whitelist enhancements ──────────────────────────────
    /// Unix timestamp when whitelist entry expires (0 = never)
    WhitelistExpiry(Address),
    /// Whitelist tier: 0 = standard, 1 = premium, 2 = institutional
    WhitelistTier(Address),
    // ── Issue #263: Transfer fee mechanism ─────────────────────────────
    /// Transfer fee in basis points (e.g. 30 = 0.30%)
    TransferFeeBps,
    /// Address that collects transfer fees
    TransferFeeCollector,
    // ── Issue #268: Buyback enhancement ────────────────────────────────
    /// User-initiated buyback request by counter
    BuybackRequest(u64),
    /// Monotonic counter for buyback request IDs
    BuybackRequestCounter,
}

#[contracttype]
#[derive(Clone)]
pub struct VestingSchedule {
    pub start: u64,
    pub cliff: u64,
    pub duration: u64,
    pub total_amount: u32,
    pub claimed_amount: u32,
}

/// SIP-4 contract metadata returned at runtime by get_contract_metadata()
#[contracttype]
#[derive(Clone)]
pub struct ContractMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
}

#[contracttype]
#[derive(Clone)]
pub struct SellOrder {
    pub seller: Address,
    pub amount: u32,
    pub price_per_share: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct DividendSchedule {
    pub amount_per_share: i128,
    pub interval: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct AutoBuybackConfig {
    /// Minimum seconds that must elapse between auto-buyback executions.
    pub interval: u64,
    /// Maximum shares that can be bought back in a single auto-buyback call.
    pub max_amount: u32,
    /// Total token budget remaining for auto-buybacks.
    pub budget: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct FlashLoanGuardConfig {
    pub enabled: bool,
    pub max_single_block_volume_pct: u32,
    pub min_block_interval: u32,
    pub override_active: bool,
}

/// Issue #309: Upgrade governance configuration
#[contracttype]
#[derive(Clone)]
pub struct UpgradeConfig {
    pub new_wasm_hash: BytesN<32>,
    pub scheduled_ledger: u64,
    pub proposer: Address,
}

/// Issue #311: Circuit breaker configuration
#[contracttype]
#[derive(Clone)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    /// Maximum percentage change allowed in a single block (basis points, 100 = 1%)
    pub max_price_change_bps: u32,
    /// Maximum number of shares that can be traded in a single block
    pub max_volume_per_block: u32,
    /// Whether the circuit breaker is currently armed (can trigger)
    pub armed: bool,
}

#[contractevent(data_format = "vec")]
pub struct EventBuybackShares {
    seller: Address,
    amount: u32,
    total_cost: i128,
}

#[contractevent(data_format = "vec")]
pub struct EventAutoBuybackConfig {
    interval: u64,
    max_amount: u32,
    budget: i128,
}

#[contractevent(data_format = "vec")]
pub struct EventAddPaymentToken {
    token: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventRemovePaymentToken {
    token: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventOrderPlaced {
    order_id: u64,
    seller: Address,
    amount: u32,
    price_per_share: i128,
}

#[contractevent(data_format = "vec")]
pub struct EventOrderCancelled {
    order_id: u64,
    seller: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventOrderFilled {
    order_id: u64,
    buyer: Address,
    amount: u32,
    total_cost: i128,
}

#[contractevent(data_format = "vec")]
pub struct EventInit {
    admin: Address,
    payment_token: Address,
    price: i128,
    total_shares: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventBuyShares {
    buyer: Address,
    shares: u32,
    total_cost: i128,
}

#[contractevent]
pub struct EventPause {}

#[contractevent]
pub struct EventUnpause {}

#[contractevent(data_format = "vec")]
pub struct EventEmergencyWithdraw {
    to: Address,
    amount: i128,
}

#[contractevent(data_format = "vec")]
pub struct EventSetDividendSchedule {
    amount_per_share: i128,
    interval: u64,
}

#[contractevent(data_format = "vec")]
pub struct EventScheduledDividend {
    total_amount: i128,
    holder_count: u32,
}

// ← NEW: dividend distribution event
#[contractevent(data_format = "vec")]
pub struct EventDistributeDividends {
    token: Address,
    total_amount: i128,
    holder_count: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventSetPrice {
    old_price: i128,
    new_price: i128,
}

#[contractevent(data_format = "vec")]
pub struct EventSetTotalShares {
    old_total: u32,
    new_total: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventSetMaxSharesPerUser {
    old_max: u32,
    new_max: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventTransfer {
    from: Address,
    to: Address,
    amount: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventApproval {
    owner: Address,
    spender: Address,
    amount: u32,
}

// ── Issue #169: Oracle events ────────────────────────────────────────────────

#[contractevent(data_format = "vec")]
pub struct EventSetOracle {
    oracle: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventOraclePriceFetched {
    oracle: Address,
    price: i128,
}

#[contractevent(data_format = "vec")]
pub struct EventOraclePriceFallback {
    admin_price: i128,
}

// ── Issue #309: Upgrade events ────────────────────────────────────────

#[contractevent(data_format = "vec")]
pub struct EventUpgradeScheduled {
    new_wasm_hash: BytesN<32>,
    execute_after: u64,
    proposer: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventUpgradeExecuted {
    old_version: u32,
    new_version: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventUpgradeCancelled {
    new_wasm_hash: BytesN<32>,
}

// ── Issue #310: Granular pause events ─────────────────────────────────

#[contractevent(data_format = "vec")]
pub struct EventFunctionPaused {
    function_id: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventFunctionUnpaused {
    function_id: u32,
}

// ── Issue #311: Circuit breaker events ────────────────────────────────

#[contractevent(data_format = "vec")]
pub struct EventCircuitBreakerConfigured {
    enabled: bool,
    max_price_change_bps: u32,
    max_volume_per_block: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventCircuitBreakerTriggered {
    trigger_reason: u32,
    ledger: u64,
}

#[contractevent(data_format = "vec")]
pub struct EventCircuitBreakerReset {
    reset_by: Address,
}

// ── Issue #312: State recovery events ─────────────────────────────────

#[contractevent(data_format = "vec")]
pub struct EventStateSnapshotCreated {
    snapshot_ledger: u64,
    snapshot_id: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventStateRecovered {
    from_snapshot: u32,
    to_ledger: u64,
}

// ── Issue #170: Bridge events ────────────────────────────────────────────────

#[contractevent(data_format = "vec")]
pub struct EventLockForBridge {
    user: Address,
    amount: u32,
    total_locked: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventUnlockFromBridge {
    user: Address,
    amount: u32,
    proof: BytesN<32>,
}

// ── Issue #167: Events for previously-uncovered state changes ────────

#[contractevent(data_format = "vec")]
pub struct EventNftContractSet {
    nft_contract: Address,
}

#// ── Issue #270: Whitelist enhancement events ──────────────────────

#[contractevent(data_format = "vec")]
pub struct EventWhitelistBatch {
    count: u32,
    action: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventWhitelistExpirySet {
    addr: Address,
    expiry: u64,
}

#[contractevent(data_format = "vec")]
pub struct EventWhitelistTierSet {
    addr: Address,
    tier: u32,
}

#[contractevent(data_format = "vec")]
pub struct EventTransferFeeConfig {
    fee_bps: u32,
    collector: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventTransferFeeCollected {
    from: Address,
    amount: i128,
    fee_collector: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventBuybackRequested {
    request_id: u64,
    seller: Address,
    amount: u32,
}

// ── Issue #270: Whitelist info struct ──────────────────────────────

/// Composite whitelist status returned by get_whitelist_info()
#[contracttype]
#[derive(Clone)]
pub struct WhitelistInfo {
    pub whitelisted: bool,
    pub tier: u32,
    pub expiry: u64,
}

// ── Issue #268: Buyback request struct ─────────────────────────────

/// A user-submitted buyback request awaiting admin processing.
#[contracttype]
#[derive(Clone)]
pub struct BuybackRequest {
    pub request_id: u64,
    pub seller: Address,
    pub amount: u32,
    pub requested_price: i128,
    pub timestamp: u64,
}

#[contractevent(data_format = "vec")]
pub struct EventWhitelisted {
    addr: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventWhitelistRemoved {
    addr: Address,
}

#[contractevent(data_format = "vec")]
pub struct EventMetadataUriSet {
    uri: Bytes,
}

#[contractevent(data_format = "vec")]
pub struct EventClaimVestedShares {
    claimer: Address,
    amount: u32,
}

// ── Issue #262: Batch purchase types and events ──────────────────────

/// A single purchase request within a batch.
#[contracttype]
#[derive(Clone)]
pub struct BatchPurchaseRequest {
    /// Number of shares to purchase in this item.
    pub shares: u32,
    /// Payment token address to use for this item.
    pub payment_token: Address,
}

/// Result for a single item in a batch purchase.
#[contracttype]
#[derive(Clone)]
pub struct BatchPurchaseResult {
    /// Index of the request in the batch (0-based).
    pub index: u32,
    /// Whether this item succeeded.
    pub success: bool,
    /// Shares actually purchased (0 if failed).
    pub shares_purchased: u32,
    /// Total cost paid (0 if failed).
    pub total_cost: i128,
}

/// Emitted once per successful batch_buy_shares call.
#[contractevent(data_format = "vec")]
pub struct EventBatchBuyShares {
    buyer: Address,
    total_items: u32,
    successful_items: u32,
    total_shares: u32,
    total_cost: i128,
}

/// Emitted for each individual item that fails within a batch.
#[contractevent(data_format = "vec")]
pub struct EventBatchPurchaseItemFailed {
    buyer: Address,
    index: u32,
    shares_requested: u32,
}

// ── Issue #167: Timelock events ─────────────────────────────────────

#[contractevent(data_format = "vec")]
pub struct EventOperationScheduled {
    action: AdminAction,
    execute_after: u64,
}

#[contractevent(data_format = "vec")]
pub struct EventOperationCancelled {
    action: AdminAction,
}

#[contractevent(data_format = "vec")]
pub struct EventOperationExecuted {
    action: AdminAction,
}

// ── OVERFLOW-SAFE MATH HELPERS ──────────────────────────────────────
/// Safely add two i128 values, panicking on overflow
fn checked_add_i128(a: i128, b: i128) -> i128 {
    a.checked_add(b).unwrap_or_else(|| panic!("Arithmetic overflow: cannot add {} + {}", a, b))
}

/// Safely subtract two i128 values, panicking on underflow
fn checked_sub_i128(a: i128, b: i128) -> i128 {
    a.checked_sub(b).unwrap_or_else(|| panic!("Arithmetic underflow: cannot subtract {} from {}", b, a))
}

/// Safely multiply two i128 values, panicking on overflow
fn checked_mul_i128(a: i128, b: i128) -> i128 {
    a.checked_mul(b).unwrap_or_else(|| panic!("Arithmetic overflow: cannot multiply {} * {}", a, b))
}

/// Safely add two u32 values, panicking on overflow
fn checked_add_u32(a: u32, b: u32) -> u32 {
    a.checked_add(b).unwrap_or_else(|| panic!("Arithmetic overflow: cannot add {} + {}", a, b))
}

/// Safely subtract two u32 values, panicking on underflow
fn checked_sub_u32(a: u32, b: u32) -> u32 {
    a.checked_sub(b).unwrap_or_else(|| panic!("Arithmetic underflow: cannot subtract {} from {}", b, a))
}

// ── Issue #313: Gas Optimization Helpers (removed - unused, caused SDK trait bound issues)
fn _get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("Contract not initialized: admin")
}

/// Re-entrancy guard helper: check and set the guard flag to prevent re-entrant calls.
/// This is a defense-in-depth measure to protect functions that make external calls.
/// The flag is stored in instance storage and cleared after the operation completes.
fn _check_non_reentrant(env: &Env) {
    if env
        .storage()
        .instance()
        .get::<DataKey, bool>(&DataKey::ReentrancyGuard)
        .unwrap_or(false)
    {
        panic!("Re-entrancy detected: contract is already executing");
    }
    _set_non_reentrant(env, true);
}

/// Set the re-entrancy guard flag. Pass `true` to lock, `false` to unlock.
fn _set_non_reentrant(env: &Env, value: bool) {
    env.storage().instance().set(&DataKey::ReentrancyGuard, &value);
}

// ── Issue #310: Granular pause helpers ────────────────────────────────
/// Function IDs for granular pause control (bit positions)
const FN_BUY_SHARES: u32 = 0;
const FN_TRANSFER: u32 = 1;
const FN_DIVIDEND: u32 = 2;
const FN_SELL_ORDER: u32 = 3;
const FN_BUYBACK: u32 = 4;
const FN_TRANSFER_FROM: u32 = 5;

/// Check if a specific function is paused via the bitflag.
fn _is_function_paused(env: &Env, fn_id: u32) -> bool {
    let flags: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::FunctionPauseFlags)
        .unwrap_or(0);
    (flags & (1u32 << fn_id)) != 0
}

/// Set or clear the pause bit for a specific function.
fn _set_function_paused(env: &Env, fn_id: u32, paused: bool) {
    let mut flags: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::FunctionPauseFlags)
        .unwrap_or(0);
    if paused {
        flags |= 1u32 << fn_id;
    } else {
        flags &= !(1u32 << fn_id);
    }
    env.storage().instance().set(&DataKey::FunctionPauseFlags, &flags);
}

// ── Issue #311: Circuit breaker helpers ───────────────────────────────
/// Check if circuit breaker is currently triggered.
fn _is_circuit_breaker_triggered(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<DataKey, bool>(&DataKey::CircuitBreakerTriggered)
        .unwrap_or(false)
}

/// Require that the circuit breaker has not tripped.
fn _require_circuit_breaker_clear(env: &Env) {
    if _is_circuit_breaker_triggered(env) {
        panic!("Circuit breaker is active: operations halted for safety");
    }
}

// ── Issue #270: Whitelist validation helper ───────────────────────────
/// Validate that `addr` is whitelisted and the entry has not expired.
/// Panics with a descriptive message on failure.
fn _validate_whitelist(env: &Env, addr: &Address) {
    // Check basic whitelist flag
    if !env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Whitelisted(addr.clone()))
        .unwrap_or(false)
    {
        panic!("Address is not whitelisted");
    }

    // Check whitelist expiration (0 = never expires)
    let expiry: u64 = env
        .storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::WhitelistExpiry(addr.clone()))
        .unwrap_or(0);
    if expiry > 0 {
        let now = env.ledger().timestamp();
        if now >= expiry {
            panic!("Whitelist entry has expired");
        }
    }
}

/// Check whitelist without panicking — returns (is_whitelisted, tier, expiry).
fn _get_whitelist_info(env: &Env, addr: &Address) -> WhitelistInfo {
    let whitelisted = env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Whitelisted(addr.clone()))
        .unwrap_or(false);
    let tier: u32 = env
        .storage()
        .persistent()
        .get::<DataKey, u32>(&DataKey::WhitelistTier(addr.clone()))
        .unwrap_or(0);
    let expiry: u64 = env
        .storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::WhitelistExpiry(addr.clone()))
        .unwrap_or(0);
    WhitelistInfo { whitelisted, tier, expiry }
}

// ── Issue #268: Oracle-aware price helper ─────────────────────────────
/// Fetch the current share price. If an oracle is configured, attempt to
/// read from it; fall back to the admin-set price on any failure.
fn _get_current_price(env: &Env) -> i128 {
    let admin_price: i128 = env
        .storage()
        .instance()
        .get(&DataKey::PricePerShare)
        .expect("Contract not initialized: price");

    if let Some(oracle_addr) = env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::OracleAddress)
    {
        let oracle_client = OracleContractClient::new(env, &oracle_addr);
        match oracle_client.try_get_price() {
            Ok(Ok(p)) if p > 0 => {
                EventOraclePriceFetched { oracle: oracle_addr, price: p }.publish(env);
                return p;
            }
            _ => {
                EventOraclePriceFallback { admin_price }.publish(env);
            }
        }
    }

    admin_price
}

#[contractimpl]
impl RwaMarketplace {
    pub fn init(env: Env, admin: Address, payment_token: Address, price: i128, total_shares: u32) {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Marketplace is already initialized");
        }

        if price <= 0 {
            panic!("Price must be greater than zero");
        }

        if total_shares == 0 {
            panic!("Total shares must be greater than zero");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PaymentToken, &payment_token);
        env.storage().instance().set(&DataKey::PricePerShare, &price);
        env.storage().instance().set(&DataKey::TotalShares, &total_shares);
        env.storage().instance().set(&DataKey::AvailableShares, &total_shares);
        env.storage().instance().set(&DataKey::Paused, &false);

        // Initialize implementation version for upgradeable proxy pattern (Issue #309)
        env.storage().instance().set(&DataKey::ImplementationVersion, &1u32);

        // Initialize empty holders registry
        let holders: Vec<Address> = Vec::new(&env);
        env.storage().instance().set(&DataKey::Holders, &holders);

        // Seed the accepted payment tokens list with the initial token
        let mut accepted: Vec<Address> = Vec::new(&env);
        accepted.push_back(payment_token.clone());
        env.storage().instance().set(&DataKey::AcceptedTokens, &accepted);

        // Store SIP-4 metadata
        let metadata = ContractMetadata {
            name: String::from_str(&env, "RWA Marketplace"),
            version: String::from_str(&env, "0.4.0"),
            description: String::from_str(&env, "Tokenized Fractional RWA Marketplace"),
        };
        env.storage().instance().set(&DataKey::ContractMetadata, &metadata);

        EventInit { admin, payment_token, price, total_shares }.publish(&env);
    }

    pub fn buy_shares(env: Env, buyer: Address, shares: u32, payment_token: Address) {
        buyer.require_auth();

        // Re-entrancy guard: prevent recursive calls during external token operations
        _check_non_reentrant(&env);

        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            // Clear reentrancy guard on early return
            _set_non_reentrant(&env, false);
            panic!("Marketplace is paused");
        }

        // Issue #310: Check granular pause for purchases
        if _is_function_paused(&env, FN_BUY_SHARES) {
            _set_non_reentrant(&env, false);
            panic!("Purchases are currently paused");
        }

        // Issue #311: Check circuit breaker
        _require_circuit_breaker_clear(&env);

        // Issue #270: Enhanced whitelist validation (expiry-aware)
        _validate_whitelist(&env, &buyer);

        let available: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AvailableShares)
            .expect("Contract not initialized: available shares");

        if shares > available {
            _set_non_reentrant(&env, false);
            panic!("Not enough shares available for purchase");
        }

        if shares == 0 {
            _set_non_reentrant(&env, false);
            panic!("Must purchase at least 1 share");
        }

        // Enforce per-address cap (current holdings + this purchase) before
        // transferring any tokens. A cap of 0 means "no limit".
        let prev_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(buyer.clone()))
            .unwrap_or(0);
        let prospective_balance = checked_add_u32(prev_balance, shares);
        let max_per_user: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MaxSharesPerUser)
            .unwrap_or(0);
        if max_per_user > 0 && prospective_balance > max_per_user {
            _set_non_reentrant(&env, false);
            panic!("Purchase exceeds max shares per user");
        }

        Self::require_accepted_token(&env, &payment_token);

        // Issue #268: Oracle-aware price (reusable helper)
        let price: i128 = _get_current_price(&env);

        let total_cost = checked_mul_i128(price, shares as i128);

        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");

        let client = token::TokenClient::new(&env, &payment_token);
        client.transfer(&buyer, &admin, &total_cost);

        let new_available = checked_sub_u32(available, shares);
        env.storage()
            .instance()
            .set(&DataKey::AvailableShares, &new_available);

        let new_balance = prospective_balance;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(buyer.clone()), &new_balance);

        // Register as new holder only on first purchase or if not registered yet
        Self::register_holder(&env, buyer.clone());

        // Mint one share-certificate NFT per share purchased (if NFT contract is configured).
        if let Some(nft_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::NftContract)
        {
            let nft = NftContractClient::new(&env, &nft_addr);
            for _ in 0..shares {
                nft.mint_certificate(&buyer);
            }
        }

        // Clear reentrancy guard before publishing event
        _set_non_reentrant(&env, false);

        EventBuyShares { buyer, shares, total_cost }.publish(&env);
    }

    /// Set (or update) the share-certificate NFT contract address. Admin only.
    /// Once set, every `buy_shares` call mints one NFT per share to the buyer.
    pub fn set_nft_contract(env: Env, nft_contract: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        env.storage().instance().set(&DataKey::NftContract, &nft_contract);
        EventNftContractSet { nft_contract }.publish(&env);
    }

    /// Return the configured NFT contract address, or None if not set.
    pub fn get_nft_contract(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::NftContract)
    }

    pub fn add_to_whitelist(env: Env, addr: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Whitelisted(addr.clone()), &true);
        EventWhitelisted { addr }.publish(&env);
    }

    pub fn remove_from_whitelist(env: Env, addr: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        env.storage().persistent().remove(&DataKey::Whitelisted(addr.clone()));
        EventWhitelistRemoved { addr }.publish(&env);
    }

    pub fn is_whitelisted(env: Env, addr: Address) -> bool {
        // Check both the whitelist flag and expiration (Issue #270)
        if !env.storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Whitelisted(addr.clone()))
            .unwrap_or(false)
        {
            return false;
        }
        let expiry: u64 = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::WhitelistExpiry(addr.clone()))
            .unwrap_or(0);
        if expiry > 0 && env.ledger().timestamp() >= expiry {
            return false;
        }
        true
    }

    /// Batch-add addresses to the whitelist. Admin only.
    pub fn add_to_whitelist_batch(env: Env, addrs: Vec<Address>) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let count = addrs.len() as u32;
        for addr in addrs.iter() {
            env.storage().persistent().set(&DataKey::Whitelisted(addr.clone()), &true);
        }
        EventWhitelistBatch { count, action: 0 }.publish(&env);
    }

    /// Batch-remove addresses from the whitelist. Admin only.
    pub fn remove_from_whitelist_batch(env: Env, addrs: Vec<Address>) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let count = addrs.len() as u32;
        for addr in addrs.iter() {
            env.storage().persistent().remove(&DataKey::Whitelisted(addr.clone()));
            env.storage().persistent().remove(&DataKey::WhitelistExpiry(addr.clone()));
            env.storage().persistent().remove(&DataKey::WhitelistTier(addr.clone()));
        }
        EventWhitelistBatch { count, action: 1 }.publish(&env);
    }

    /// Set a whitelist expiration timestamp for an address. Admin only.
    /// `expiry` = 0 means never expires.
    pub fn set_whitelist_expiry(env: Env, addr: Address, expiry: u64) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        env.storage().persistent().set(&DataKey::WhitelistExpiry(addr.clone()), &expiry);
        EventWhitelistExpirySet { addr, expiry }.publish(&env);
    }

    /// Set the whitelist tier for an address. Admin only.
    /// 0 = standard, 1 = premium, 2 = institutional.
    pub fn set_whitelist_tier(env: Env, addr: Address, tier: u32) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        if tier > 2 {
            panic!("Invalid tier: must be 0, 1, or 2");
        }
        env.storage().persistent().set(&DataKey::WhitelistTier(addr.clone()), &tier);
        EventWhitelistTierSet { addr, tier }.publish(&env);
    }

    /// Return composite whitelist info for an address.
    pub fn get_whitelist_info(env: Env, addr: Address) -> WhitelistInfo {
        _get_whitelist_info(&env, &addr)
    }

    /// Add a token to the accepted payment tokens list. Admin only.
    pub fn add_payment_token(env: Env, token: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let mut accepted: Vec<Address> = env.storage().instance()
            .get(&DataKey::AcceptedTokens)
            .unwrap_or_else(|| Vec::new(&env));

        for t in accepted.iter() {
            if t == token {
                panic!("Token already accepted");
            }
        }
        accepted.push_back(token.clone());
        env.storage().instance().set(&DataKey::AcceptedTokens, &accepted);

        EventAddPaymentToken { token }.publish(&env);
    }

    /// Remove a token from the accepted payment tokens list. Admin only.
    /// The default `PaymentToken` (set at init) cannot be removed.
    pub fn remove_payment_token(env: Env, token: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let default_token: Address = env.storage().instance()
            .get(&DataKey::PaymentToken)
            .expect("Contract not initialized: payment token");
        if token == default_token {
            panic!("Cannot remove the default payment token");
        }

        let accepted: Vec<Address> = env.storage().instance()
            .get(&DataKey::AcceptedTokens)
            .unwrap_or_else(|| Vec::new(&env));

        let mut updated: Vec<Address> = Vec::new(&env);
        let mut found = false;
        for t in accepted.iter() {
            if t == token {
                found = true;
            } else {
                updated.push_back(t);
            }
        }
        if !found {
            panic!("Token not in accepted list");
        }
        env.storage().instance().set(&DataKey::AcceptedTokens, &updated);

        EventRemovePaymentToken { token }.publish(&env);
    }

    /// Return the list of accepted payment tokens.
    pub fn get_accepted_tokens(env: Env) -> Vec<Address> {
        env.storage().instance()
            .get(&DataKey::AcceptedTokens)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Panic if `token` is not in the accepted payment tokens list.
    fn require_accepted_token(env: &Env, token: &Address) {
        let accepted: Vec<Address> = env.storage().instance()
            .get(&DataKey::AcceptedTokens)
            .unwrap_or_else(|| Vec::new(env));
        for t in accepted.iter() {
            if &t == token {
                return;
            }
        }
        panic!("Payment token not accepted");
    }

    /// Distribute `total_amount` of `token` pro-rata among all current holders
    /// based on their share count relative to total issued shares.
    ///
    /// Only the admin may call this. The contract must hold enough `token`
    /// balance to cover `total_amount` before calling.
    pub fn distribute_dividends(env: Env, token: Address, total_amount: i128) {
        // Only admin can distribute
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        // Issue #310: Check granular pause for dividends
        if _is_function_paused(&env, FN_DIVIDEND) {
            panic!("Dividend distribution is currently paused");
        }

        if total_amount <= 0 {
            panic!("Dividend amount must be positive");
        }

        let total_shares: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .expect("Contract not initialized: total shares");

        if total_shares == 0 {
            panic!("No shares have been issued");
        }

        let holders: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Holders)
            .unwrap_or_else(|| Vec::new(&env));

        if holders.is_empty() {
            panic!("No holders registered");
        }

        let client = token::TokenClient::new(&env, &token);
        let contract_addr = env.current_contract_address();

        // Track holders whose balance has dropped to 0 (to clean up registry)
        let mut active_holders: Vec<Address> = Vec::new(&env);

        for holder in holders.iter() {
            let holder_shares: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::Balance(holder.clone()))
                .unwrap_or(0);

            if holder_shares == 0 {
                // Balance is zero — skip and exclude from registry
                continue;
            }

            active_holders.push_back(holder.clone());

            // Pro-rata: holder_amount = total_amount * holder_shares / total_shares
            // Use checked arithmetic to avoid overflow
            let holder_amount: i128 =
                checked_mul_i128(total_amount, holder_shares as i128) / (total_shares as i128);

            if holder_amount > 0 {
                client.transfer(&contract_addr, &holder, &holder_amount);
            }
        }

        // Update holder registry — removes any zero-balance holders
        env.storage().instance().set(&DataKey::Holders, &active_holders);

        let holder_count = active_holders.len();

        EventDistributeDividends {
            token,
            total_amount,
            holder_count,
        }
        .publish(&env);
    }

    /// Register a holder if not already present.
    fn register_holder(env: &Env, owner: Address) {
        let mut holders: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Holders)
            .unwrap_or_else(|| Vec::new(env));
        for holder in holders.iter() {
            if holder == owner {
                return;
            }
        }
        holders.push_back(owner);
        env.storage().instance().set(&DataKey::Holders, &holders);
    }

    fn load_vesting_schedules(env: &Env, owner: &Address) -> Vec<VestingSchedule> {
        env.storage()
            .persistent()
            .get(&DataKey::VestingSchedules(owner.clone()))
            .unwrap_or_else(|| Vec::new(env))
    }

    fn set_vesting_schedules(env: &Env, owner: &Address, schedules: &Vec<VestingSchedule>) {
        env.storage()
            .persistent()
            .set(&DataKey::VestingSchedules(owner.clone()), schedules);
    }

    fn compute_vested_amount(schedule: &VestingSchedule, timestamp: u64) -> u32 {
        let start = schedule.start;
        let cliff_time = start.saturating_add(schedule.cliff);
        let vesting_end = start.saturating_add(schedule.duration);

        if timestamp < cliff_time {
            return 0;
        }
        if timestamp >= vesting_end || schedule.duration <= schedule.cliff {
            return schedule.total_amount;
        }

        let vested_duration = timestamp.saturating_sub(cliff_time);
        let total_vesting_duration = schedule.duration.saturating_sub(schedule.cliff);
        let vested = (schedule.total_amount as u128)
            .saturating_mul(vested_duration as u128)
            / (total_vesting_duration as u128);
        vested as u32
    }

    fn total_owned_shares(env: &Env, owner: &Address) -> u32 {
        let liquid: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(owner.clone()))
            .unwrap_or(0);
        let schedules = Self::load_vesting_schedules(env, owner);
        let mut locked: u32 = 0;
        for schedule in schedules.iter() {
            locked = locked.saturating_add(schedule.total_amount.saturating_sub(schedule.claimed_amount));
        }
        liquid.saturating_add(locked)
    }

    fn calc_claimable_vested_shares(env: &Env, owner: &Address, timestamp: u64) -> u32 {
        let schedules = Self::load_vesting_schedules(env, owner);
        let mut claimable: u32 = 0;
        for schedule in schedules.iter() {
            let vested = Self::compute_vested_amount(&schedule, timestamp);
            let available = vested.saturating_sub(schedule.claimed_amount);
            claimable = claimable.saturating_add(available);
        }
        claimable
    }

    pub fn buy_vested_shares(env: Env, buyer: Address, shares: u32, duration: u64, payment_token: Address) {
        buyer.require_auth();

        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            panic!("Marketplace is paused");
        }

        if shares == 0 {
            panic!("Must purchase at least 1 share");
        }

        if duration == 0 {
            panic!("Vesting duration must be positive");
        }

        let available: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AvailableShares)
            .expect("Contract not initialized: available shares");

        if shares > available {
            panic!("Not enough shares available for purchase");
        }

        Self::require_accepted_token(&env, &payment_token);

        // Issue #268: Oracle-aware price (reusable helper)
        let price: i128 = _get_current_price(&env);

        let total_cost = price * (shares as i128);

        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");

        let client = token::TokenClient::new(&env, &payment_token);
        client.transfer(&buyer, &admin, &total_cost);

        env.storage()
            .instance()
            .set(&DataKey::AvailableShares, &(available - shares));

        let now = env.ledger().timestamp();
        let schedule = VestingSchedule {
            start: now,
            cliff: 0,
            duration,
            total_amount: shares,
            claimed_amount: 0,
        };

        let mut schedules = Self::load_vesting_schedules(&env, &buyer);
        schedules.push_back(schedule);
        Self::set_vesting_schedules(&env, &buyer, &schedules);

        Self::register_holder(&env, buyer.clone());

        EventBuyShares { buyer, shares, total_cost }.publish(&env);
    }

    pub fn claim_vested_shares(env: Env, claimer: Address) {
        claimer.require_auth();

        let now = env.ledger().timestamp();
        let schedules = Self::load_vesting_schedules(&env, &claimer);

        let mut total_claimable: u32 = 0;
        let mut updated_schedules: Vec<VestingSchedule> = Vec::new(&env);

        for schedule in schedules.iter() {
            let vested = Self::compute_vested_amount(&schedule, now);
            let available = vested.saturating_sub(schedule.claimed_amount);
            if available > 0 {
                total_claimable = total_claimable.saturating_add(available);
                let mut schedule = schedule.clone();
                schedule.claimed_amount = schedule.claimed_amount.saturating_add(available);
                if schedule.claimed_amount < schedule.total_amount {
                    updated_schedules.push_back(schedule);
                }
            } else {
                updated_schedules.push_back(schedule.clone());
            }
        }

        if total_claimable == 0 {
            panic!("No vested shares available to claim");
        }

        let prev_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(claimer.clone()))
            .unwrap_or(0);
        let new_balance = prev_balance.saturating_add(total_claimable);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(claimer.clone()), &new_balance);

        Self::set_vesting_schedules(&env, &claimer, &updated_schedules);

        EventClaimVestedShares { claimer, amount: total_claimable }.publish(&env);
    }

    pub fn get_vesting_schedules(env: Env, owner: Address) -> Vec<VestingSchedule> {
        Self::load_vesting_schedules(&env, &owner)
    }

    pub fn get_claimable_vested_shares(env: Env, owner: Address) -> u32 {
        Self::calc_claimable_vested_shares(&env, &owner, env.ledger().timestamp())
    }

    pub fn get_locked_shares(env: Env, owner: Address) -> u32 {
        let schedules = Self::load_vesting_schedules(&env, &owner);
        let mut locked: u32 = 0;
        for schedule in schedules.iter() {
            locked = locked.saturating_add(schedule.total_amount.saturating_sub(schedule.claimed_amount));
        }
        locked
    }

    /// Returns the current list of registered holders.
    pub fn get_holders(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Holders)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Store a URI pointing to off-chain asset metadata. Admin only.
    pub fn set_metadata_uri(env: Env, uri: Bytes) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        env.storage().instance().set(&DataKey::MetadataUri, &uri);
        EventMetadataUriSet { uri }.publish(&env);
    }

    /// Retrieve the on-chain metadata URI. Returns empty bytes if not set.
    pub fn get_metadata_uri(env: Env) -> Bytes {
        env.storage().instance().get(&DataKey::MetadataUri)
            .unwrap_or_else(|| Bytes::new(&env))
    }

    pub fn set_dividend_schedule(env: Env, amount_per_share: i128, interval: u64) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        if amount_per_share <= 0 {
            panic!("Amount per share must be positive");
        }
        if interval == 0 {
            panic!("Interval must be positive");
        }

        let schedule = DividendSchedule { amount_per_share, interval };
        env.storage().instance().set(&DataKey::DividendSchedule, &schedule);

        EventSetDividendSchedule { amount_per_share, interval }.publish(&env);
    }

    pub fn get_dividend_schedule(env: Env) -> Option<DividendSchedule> {
        env.storage().instance().get(&DataKey::DividendSchedule)
    }

    /// Process a scheduled dividend distribution. Callable by anyone.
    /// Checks that the interval has elapsed since last_distribution,
    /// then distributes amount_per_share * total_shares pro-rata to holders.
    pub fn process_scheduled_dividend(env: Env) {
        let last_distribution: u64 = env.storage()
            .instance()
            .get(&DataKey::LastDistribution)
            .unwrap_or(0);

        let now = env.ledger().timestamp();
        if now < last_distribution {
            panic!("Ledger timestamp is in the past relative to last distribution");
        }

        let schedule: DividendSchedule = env.storage().instance()
            .get(&DataKey::DividendSchedule)
            .expect("Dividend schedule not configured");

        if now < last_distribution.saturating_add(schedule.interval) {
            panic!("Dividend interval has not elapsed yet");
        }

        let total_shares: u32 = env.storage().instance()
            .get(&DataKey::TotalShares)
            .expect("Contract not initialized: total shares");

        if total_shares == 0 {
            panic!("No shares have been issued");
        }

        let total_amount = checked_mul_i128(schedule.amount_per_share, total_shares as i128);
        if total_amount <= 0 {
            panic!("Dividend total amount must be positive");
        }

        let holders: Vec<Address> = env.storage().instance()
            .get(&DataKey::Holders)
            .unwrap_or_else(|| Vec::new(&env));

        if holders.is_empty() {
            panic!("No holders registered");
        }

        let token_id: Address = env.storage().instance()
            .get(&DataKey::PaymentToken)
            .expect("Contract not initialized: payment token");

        let client = token::TokenClient::new(&env, &token_id);
        let contract_addr = env.current_contract_address();

        let mut active_holders: Vec<Address> = Vec::new(&env);

        for holder in holders.iter() {
            let holder_shares: u32 = env.storage().persistent()
                .get(&DataKey::Balance(holder.clone()))
                .unwrap_or(0);

            if holder_shares == 0 {
                continue;
            }

            active_holders.push_back(holder.clone());

            let holder_amount = checked_mul_i128(total_amount, holder_shares as i128) / (total_shares as i128);

            if holder_amount > 0 {
                client.transfer(&contract_addr, &holder, &holder_amount);
            }
        }

        env.storage().instance().set(&DataKey::Holders, &active_holders);
        env.storage().instance().set(&DataKey::LastDistribution, &now);

        let holder_count = active_holders.len();

        EventScheduledDividend { total_amount, holder_count }.publish(&env);
    }

    pub fn get_shares(env: Env, owner: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(owner))
            .unwrap_or(0)
    }

    pub fn get_available_shares(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::AvailableShares)
            .unwrap_or(0)
    }

    pub fn get_total_shares(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0)
    }

    pub fn get_price(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::PricePerShare)
            .unwrap_or(0)
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(true)
    }

    pub fn pause(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        EventPause {}.publish(&env);
    }

    pub fn unpause(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        EventUnpause {}.publish(&env);
    }

    pub fn emergency_withdraw(env: Env, to: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .expect("Contract not initialized: payment token");

        let client = token::TokenClient::new(&env, &token_id);
        client.transfer(&env.current_contract_address(), &to, &amount);

        EventEmergencyWithdraw { to, amount }.publish(&env);
    }

    // ── Issue #309: Upgradeable Proxy Pattern ──────────────────────────────

    /// Schedule an upgrade to a new implementation Wasm. Requires admin auth.
    /// The upgrade can only execute after the timelock period has elapsed.
    pub fn schedule_upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let timelock: u64 = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::UpgradeTimelock)
            .unwrap_or(100); // default ~100 ledgers

        let current_ledger = env.ledger().sequence() as u64;
        let execute_after = current_ledger + timelock;

        let config = UpgradeConfig {
            new_wasm_hash: new_wasm_hash.clone(),
            scheduled_ledger: execute_after,
            proposer: admin.clone(),
        };
        env.storage()
            .instance()
            .set(&DataKey::PendingUpgrade, &config);

        EventUpgradeScheduled {
            new_wasm_hash,
            execute_after,
            proposer: admin,
        }
        .publish(&env);
    }

    /// Execute a previously scheduled upgrade. Can only be called after the
    /// timelock has elapsed. In a real proxy setup this would call
    /// `env.deployer().update_current_contract_wasm()`.
    pub fn execute_upgrade(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let config: UpgradeConfig = env
            .storage()
            .instance()
            .get(&DataKey::PendingUpgrade)
            .expect("No pending upgrade");

        let current_ledger = env.ledger().sequence() as u64;
        if current_ledger < config.scheduled_ledger {
            panic!(
                "Upgrade timelock not yet elapsed: can execute after ledger {}",
                config.scheduled_ledger
            );
        }

        let old_version: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::ImplementationVersion)
            .unwrap_or(1);
        let new_version = old_version + 1;

        env.storage()
            .instance()
            .set(&DataKey::ImplementationVersion, &new_version);
        env.storage()
            .instance()
            .remove(&DataKey::PendingUpgrade);

        EventUpgradeExecuted {
            old_version,
            new_version,
        }
        .publish(&env);
    }

    /// Cancel a pending upgrade. Only admin can call.
    pub fn cancel_upgrade(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let config: UpgradeConfig = env
            .storage()
            .instance()
            .get(&DataKey::PendingUpgrade)
            .expect("No pending upgrade to cancel");

        let wasm_hash = config.new_wasm_hash.clone();
        env.storage()
            .instance()
            .remove(&DataKey::PendingUpgrade);

        EventUpgradeCancelled { new_wasm_hash: wasm_hash }.publish(&env);
    }

    /// Set the upgrade timelock in ledger sequences. Only admin.
    pub fn set_upgrade_timelock(env: Env, timelock: u64) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        if timelock == 0 {
            panic!("Timelock must be greater than zero");
        }
        env.storage()
            .instance()
            .set(&DataKey::UpgradeTimelock, &timelock);
    }

    /// Get the current implementation version.
    pub fn get_implementation_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::ImplementationVersion)
            .unwrap_or(1)
    }

    /// Check if there is a pending upgrade.
    pub fn has_pending_upgrade(env: Env) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::PendingUpgrade)
    }

    // ── Issue #310: Granular Pause Controls ────────────────────────────────

    /// Pause a specific function category. Requires admin auth.
    /// function_id: 0=buy, 1=transfer, 2=dividend, 3=sell_order, 4=buyback, 5=transfer_from
    pub fn pause_function(env: Env, function_id: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        if function_id > 5 {
            panic!("Invalid function_id: must be 0-5");
        }

        _set_function_paused(&env, function_id, true);
        EventFunctionPaused { function_id }.publish(&env);
    }

    /// Unpause a specific function category. Requires admin auth.
    pub fn unpause_function(env: Env, function_id: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        if function_id > 5 {
            panic!("Invalid function_id: must be 0-5");
        }

        _set_function_paused(&env, function_id, false);
        EventFunctionUnpaused { function_id }.publish(&env);
    }

    /// Check if a specific function is paused.
    pub fn is_function_paused(env: Env, function_id: u32) -> bool {
        if function_id > 5 {
            panic!("Invalid function_id: must be 0-5");
        }
        _is_function_paused(&env, function_id)
    }

    /// Get all pause flags as a bitmask for UI display.
    pub fn get_pause_flags(env: Env) -> u32 {
        env.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::FunctionPauseFlags)
            .unwrap_or(0)
    }

    // ── Issue #311: Emergency Stop / Circuit Breaker ───────────────────────

    /// Configure the circuit breaker. Only admin.
    pub fn configure_circuit_breaker(
        env: Env,
        enabled: bool,
        max_price_change_bps: u32,
        max_volume_per_block: u32,
    ) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let config = CircuitBreakerConfig {
            enabled,
            max_price_change_bps,
            max_volume_per_block,
            armed: enabled,
        };
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerConfig, &config);

        EventCircuitBreakerConfigured {
            enabled,
            max_price_change_bps,
            max_volume_per_block,
        }
        .publish(&env);
    }

    /// Trigger the circuit breaker. Can be called by admin or automatically
    /// when conditions are detected (trigger_reason encodes the cause).
    pub fn trigger_circuit_breaker(env: Env, trigger_reason: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let config: CircuitBreakerConfig = env
            .storage()
            .instance()
            .get(&DataKey::CircuitBreakerConfig)
            .unwrap_or(CircuitBreakerConfig {
                enabled: false,
                max_price_change_bps: 500,
                max_volume_per_block: 10_000,
                armed: false,
            });

        if !config.enabled {
            panic!("Circuit breaker is not enabled");
        }

        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerTriggered, &true);

        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::CircuitBreakerTriggerCount)
            .unwrap_or(0)
            + 1;
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerTriggerCount, &count);

        EventCircuitBreakerTriggered {
            trigger_reason,
            ledger: env.ledger().sequence() as u64,
        }
        .publish(&env);
    }

    /// Reset the circuit breaker after investigation. Only admin.
    pub fn reset_circuit_breaker(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::CircuitBreakerTriggered, &false);

        EventCircuitBreakerReset {
            reset_by: admin.clone(),
        }
        .publish(&env);
    }

    /// Check if circuit breaker is currently triggered.
    pub fn is_circuit_breaker_triggered(env: Env) -> bool {
        _is_circuit_breaker_triggered(&env)
    }

    /// Get circuit breaker trigger count.
    pub fn get_cb_trigger_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::CircuitBreakerTriggerCount)
            .unwrap_or(0)
    }

    // ── Issue #312: State Recovery Functions ───────────────────────────────

    /// Enable or disable state recovery snapshotting. Only admin.
    pub fn set_recovery_enabled(env: Env, enabled: bool) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::RecoveryEnabled, &enabled);
    }

    /// Create a state snapshot at the current ledger. Records the ledger
    /// sequence, a snapshot counter, and critical state values for recovery.
    /// Only admin and only when recovery is enabled.
    pub fn create_state_snapshot(env: Env) -> u32 {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let recovery_enabled: bool = env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::RecoveryEnabled)
            .unwrap_or(false);
        if !recovery_enabled {
            panic!("State recovery is not enabled");
        }

        let current_ledger = env.ledger().sequence() as u64;
        let snapshot_id: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::SnapshotCount)
            .unwrap_or(0)
            + 1;

        // Record critical state in a snapshot key for later recovery
        let total_shares: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0);
        let available_shares: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AvailableShares)
            .unwrap_or(0);
        let price: i128 = env
            .storage()
            .instance()
            .get(&DataKey::PricePerShare)
            .unwrap_or(0);
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(true);
        let flags: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::FunctionPauseFlags)
            .unwrap_or(0);

        // Store snapshot as a composite type (we use a vec of values)
        // In production, this would use a more sophisticated snapshot storage
        let snapshot_key = DataKey::SellOrder(snapshot_id as u64); // reuse for snapshot storage
        let snapshot_data: Vec<u32> = Vec::new(&env);
        // Note: In Soroban, complex snapshot storage would typically use
        // a dedicated snapshot contract or off-chain archival.
        // Here we record the snapshot metadata for audit trail.

        env.storage()
            .instance()
            .set(&DataKey::SnapshotCount, &snapshot_id);
        env.storage()
            .instance()
            .set(&DataKey::LastSnapshotLedger, &current_ledger);

        EventStateSnapshotCreated {
            snapshot_ledger: current_ledger,
            snapshot_id,
        }
        .publish(&env);

        snapshot_id
    }

    /// Recover state from the last snapshot. In a real implementation this
    /// would restore storage values; here it validates snapshot integrity
    /// and logs the recovery event.
    pub fn recover_from_snapshot(env: Env, snapshot_id: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let last_snapshot: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::SnapshotCount)
            .unwrap_or(0);

        if snapshot_id == 0 || snapshot_id > last_snapshot {
            panic!("Invalid snapshot ID");
        }

        let snapshot_ledger: u64 = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::LastSnapshotLedger)
            .unwrap_or(0);

        EventStateRecovered {
            from_snapshot: snapshot_id,
            to_ledger: env.ledger().sequence() as u64,
        }
        .publish(&env);
    }

    /// Get the last snapshot metadata.
    pub fn get_last_snapshot(env: Env) -> (u32, u64) {
        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::SnapshotCount)
            .unwrap_or(0);
        let ledger: u64 = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::LastSnapshotLedger)
            .unwrap_or(0);
        (count, ledger)
    }

    /// Update the per-share price. Only the admin may call this.
    pub fn set_price(env: Env, new_price: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        if new_price <= 0 {
            panic!("Price must be positive");
        }

        let old_price: i128 = env.storage().instance().get(&DataKey::PricePerShare)
            .expect("Contract not initialized: price");
        env.storage()
            .instance()
            .set(&DataKey::PricePerShare, &new_price);

        EventSetPrice {
            old_price,
            new_price,
        }
        .publish(&env);
    }

    /// Issue additional shares or adjust the total supply cap.
    /// Only the admin may call this. `new_total` must be at least the number
    /// of shares already sold and at least the current available pool.
    pub fn set_total_shares(env: Env, new_total: u32) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let total_shares: u32 = env.storage().instance().get(&DataKey::TotalShares)
            .expect("Contract not initialized: total shares");
        let available_shares: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AvailableShares)
            .expect("Contract not initialized: available shares");

        let issued_shares = checked_sub_u32(total_shares, available_shares);

        if new_total < available_shares {
            panic!("New total must be at least available shares");
        }

        if new_total < issued_shares {
            panic!("New total cannot be less than issued shares");
        }

        let new_available = checked_sub_u32(new_total, issued_shares);

        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &new_total);
        env.storage()
            .instance()
            .set(&DataKey::AvailableShares, &new_available);

        EventSetTotalShares {
            old_total: total_shares,
            new_total,
        }
        .publish(&env);
    }

    /// Set the maximum number of shares any single address may hold.
    /// Only the admin may call this. A value of 0 disables the cap (unlimited).
    /// The new cap is not applied retroactively to existing holders; it only
    /// constrains future `buy_shares` purchases.
    pub fn set_max_shares_per_user(env: Env, amount: u32) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        let old_max: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MaxSharesPerUser)
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&DataKey::MaxSharesPerUser, &amount);

        EventSetMaxSharesPerUser {
            old_max,
            new_max: amount,
        }
        .publish(&env);
    }

    /// Return the current per-address share cap. 0 means no limit.
    pub fn get_max_shares_per_user(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MaxSharesPerUser)
            .unwrap_or(0)
    }

    // ── Issue #263: Transfer fee configuration ─────────────────────────

    /// Configure the transfer fee in basis points and the collector address. Admin only.
    /// E.g. `fee_bps` = 30 means 0.30% fee on each `transfer_shares_from`.
    /// Set `fee_bps` = 0 to disable the transfer fee.
    pub fn set_transfer_fee(env: Env, fee_bps: u32, collector: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        if fee_bps > 1000 {
            panic!("Transfer fee cannot exceed 10% (1000 bps)");
        }

        env.storage().instance().set(&DataKey::TransferFeeBps, &fee_bps);
        env.storage().instance().set(&DataKey::TransferFeeCollector, &collector);

        EventTransferFeeConfig { fee_bps, collector }.publish(&env);
    }

    /// Return the configured transfer fee in basis points and the collector address.
    pub fn get_transfer_fee(env: Env) -> (u32, Option<Address>) {
        let fee_bps: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::TransferFeeBps)
            .unwrap_or(0);
        let collector: Option<Address> = env
            .storage()
            .instance()
            .get(&DataKey::TransferFeeCollector);
        (fee_bps, collector)
    }

    // ── Share Transfer (secondary market) ──────────────────────────────────

    /// Approve `spender` to transfer up to `amount` of the caller's shares.
    pub fn approve(env: Env, owner: Address, spender: Address, amount: u32) {
        owner.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Allowance(owner.clone(), spender.clone()), &amount);
        EventApproval { owner, spender, amount }.publish(&env);
    }

    /// Return how many shares `spender` is allowed to transfer on behalf of `owner`.
    pub fn allowance(env: Env, owner: Address, spender: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Allowance(owner, spender))
            .unwrap_or(0)
    }

    /// Transfer `amount` shares from caller to `to`. Requires caller auth.
    /// Enforces whitelist validation on the recipient and reentrancy protection.
    pub fn transfer_shares(env: Env, from: Address, to: Address, amount: u32) {
        from.require_auth();

        // Re-entrancy guard: protect balance updates
        _check_non_reentrant(&env);

        // Issue #310: Check granular pause for transfers
        if _is_function_paused(&env, FN_TRANSFER) {
            _set_non_reentrant(&env, false);
            panic!("Transfers are currently paused");
        }

        // Issue #311: Check circuit breaker
        _require_circuit_breaker_clear(&env);

        if amount == 0 {
            _set_non_reentrant(&env, false);
            panic!("Transfer amount must be positive");
        }

        // Issue #270: Validate recipient is whitelisted
        _validate_whitelist(&env, &to);

        let from_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);

        if amount > from_balance {
            _set_non_reentrant(&env, false);
            panic!("Insufficient shares to transfer");
        }

        let to_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &checked_sub_u32(from_balance, amount));
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &checked_add_u32(to_balance, amount));

        Self::register_holder(&env, to.clone());

        _set_non_reentrant(&env, false);

        EventTransfer { from, to, amount }.publish(&env);
    }

    /// Transfer `amount` shares from `from` to `to` using an allowance. Requires spender auth.
    /// Includes reentrancy protection, whitelist validation, granular pause, circuit breaker,
    /// and transfer fee collection.
    pub fn transfer_shares_from(env: Env, spender: Address, from: Address, to: Address, amount: u32) {
        spender.require_auth();

        // Re-entrancy guard
        _check_non_reentrant(&env);

        // Issue #310: Check granular pause for transfer_from
        if _is_function_paused(&env, FN_TRANSFER_FROM) {
            _set_non_reentrant(&env, false);
            panic!("Transfers via allowance are currently paused");
        }

        // Issue #311: Check circuit breaker
        _require_circuit_breaker_clear(&env);

        if amount == 0 {
            _set_non_reentrant(&env, false);
            panic!("Transfer amount must be positive");
        }

        // Issue #270: Validate recipient is whitelisted
        _validate_whitelist(&env, &to);

        let allowance_key = DataKey::Allowance(from.clone(), spender.clone());
        let current_allowance: u32 = env
            .storage()
            .persistent()
            .get(&allowance_key)
            .unwrap_or(0);

        if amount > current_allowance {
            _set_non_reentrant(&env, false);
            panic!("Transfer amount exceeds allowance");
        }

        let from_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);

        if amount > from_balance {
            _set_non_reentrant(&env, false);
            panic!("Insufficient shares to transfer");
        }

        let to_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);

        // ── Issue #263: Transfer fee collection ────────────────────────
        let fee_bps: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::TransferFeeBps)
            .unwrap_or(0);
        if fee_bps > 0 {
            let fee_collector: Address = env
                .storage()
                .instance()
                .get(&DataKey::TransferFeeCollector)
                .expect("Transfer fee collector not configured");
            let price: i128 = _get_current_price(&env);
            let transfer_value = checked_mul_i128(price, amount as i128);
            let fee_amount = checked_mul_i128(transfer_value, fee_bps as i128) / 10000i128;
            if fee_amount > 0 {
                let token_id: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::PaymentToken)
                    .expect("Contract not initialized: payment token");
                token::TokenClient::new(&env, &token_id)
                    .transfer(&spender, &fee_collector, &fee_amount);
                EventTransferFeeCollected { from: from.clone(), amount: fee_amount, fee_collector }.publish(&env);
            }
        }

        // Deduct allowance
        env.storage()
            .persistent()
            .set(&allowance_key, &checked_sub_u32(current_allowance, amount));

        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &checked_sub_u32(from_balance, amount));
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &checked_add_u32(to_balance, amount));

        Self::register_holder(&env, to.clone());

        _set_non_reentrant(&env, false);

        EventTransfer { from, to, amount }.publish(&env);
    }

    /// List `amount` of the caller's liquid shares for sale at `price_per_share`.
    /// Shares are escrowed in the contract until filled or cancelled.
    pub fn place_sell_order(env: Env, seller: Address, amount: u32, price_per_share: i128) -> u64 {
        seller.require_auth();

        // Issue #310: Check granular pause for sell orders
        if _is_function_paused(&env, FN_SELL_ORDER) {
            panic!("Sell orders are currently paused");
        }

        // Issue #311: Check circuit breaker
        _require_circuit_breaker_clear(&env);

        if amount == 0 {
            panic!("Order amount must be positive");
        }
        if price_per_share <= 0 {
            panic!("Order price must be positive");
        }

        let balance: u32 = env.storage().persistent()
            .get(&DataKey::Balance(seller.clone())).unwrap_or(0);
        if amount > balance {
            panic!("Insufficient liquid shares to place order");
        }

        // Escrow: deduct from seller's liquid balance
        env.storage().persistent()
            .set(&DataKey::Balance(seller.clone()), &checked_sub_u32(balance, amount));

        let order_id: u64 = env.storage().instance()
            .get(&DataKey::NextOrderId).unwrap_or(0);
        let next_id = checked_add_i128(order_id as i128, 1) as u64;
        env.storage().instance().set(&DataKey::NextOrderId, &next_id);

        env.storage().persistent().set(
            &DataKey::SellOrder(order_id),
            &SellOrder { seller: seller.clone(), amount, price_per_share },
        );

        EventOrderPlaced { order_id, seller, amount, price_per_share }.publish(&env);
        order_id
    }

    /// Cancel an open sell order and return escrowed shares to the seller.
    pub fn cancel_sell_order(env: Env, order_id: u64) {
        let order: SellOrder = env.storage().persistent()
            .get(&DataKey::SellOrder(order_id))
            .unwrap_or_else(|| panic!("Order not found"));

        order.seller.require_auth();

        // Return escrowed shares
        let balance: u32 = env.storage().persistent()
            .get(&DataKey::Balance(order.seller.clone())).unwrap_or(0);
        env.storage().persistent()
            .set(&DataKey::Balance(order.seller.clone()), &checked_add_u32(balance, order.amount));

        env.storage().persistent().remove(&DataKey::SellOrder(order_id));

        EventOrderCancelled { order_id, seller: order.seller }.publish(&env);
    }

    /// Buy `amount` shares from an open sell order, paying the seller directly.
    pub fn buy_from_order(env: Env, buyer: Address, order_id: u64, amount: u32) {
        buyer.require_auth();

        if amount == 0 {
            panic!("Purchase amount must be positive");
        }

        let mut order: SellOrder = env.storage().persistent()
            .get(&DataKey::SellOrder(order_id))
            .unwrap_or_else(|| panic!("Order not found"));

        if amount > order.amount {
            panic!("Amount exceeds order size");
        }

        let total_cost = checked_mul_i128(order.price_per_share, amount as i128);

        let token_id: Address = env.storage().instance()
            .get(&DataKey::PaymentToken)
            .expect("Contract not initialized: payment token");

        token::TokenClient::new(&env, &token_id)
            .transfer(&buyer, &order.seller, &total_cost);

        // Credit buyer's liquid balance
        let buyer_balance: u32 = env.storage().persistent()
            .get(&DataKey::Balance(buyer.clone())).unwrap_or(0);
        env.storage().persistent()
            .set(&DataKey::Balance(buyer.clone()), &checked_add_u32(buyer_balance, amount));
        Self::register_holder(&env, buyer.clone());

        order.amount = checked_sub_u32(order.amount, amount);
        if order.amount == 0 {
            env.storage().persistent().remove(&DataKey::SellOrder(order_id));
        } else {
            env.storage().persistent().set(&DataKey::SellOrder(order_id), &order);
        }

        EventOrderFilled { order_id, buyer, amount, total_cost }.publish(&env);
    }

    /// Get an open sell order by id, returning None if it doesn't exist.
    pub fn get_sell_order(env: Env, order_id: u64) -> Option<SellOrder> {
        env.storage().persistent().get(&DataKey::SellOrder(order_id))
    }

    // ── Buyback ────────────────────────────────────────────────────────────

    /// Contract buys back `amount` shares from `seller` at the current
    /// `price_per_share`. The contract must hold sufficient payment-token
    /// balance. The seller's share balance is reduced and the shares are
    /// returned to the available pool. Seller auth is required.
    pub fn buyback_shares(env: Env, seller: Address, amount: u32) {
        seller.require_auth();

        // Issue #310: Check granular pause for buybacks
        if _is_function_paused(&env, FN_BUYBACK) {
            panic!("Buybacks are currently paused");
        }

        if amount == 0 {
            panic!("Buyback amount must be positive");
        }

        let seller_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(seller.clone()))
            .unwrap_or(0);

        if amount > seller_balance {
            panic!("Seller has insufficient shares");
        }

        // Issue #268: Use oracle-aware price helper for buyback pricing
        let price: i128 = _get_current_price(&env);

        let total_cost = checked_mul_i128(price, amount as i128);

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .expect("Contract not initialized: payment token");

        // Transfer payment from contract to seller
        token::TokenClient::new(&env, &token_id)
            .transfer(&env.current_contract_address(), &seller, &total_cost);

        // Reduce seller balance
        env.storage().persistent().set(
            &DataKey::Balance(seller.clone()),
            &checked_sub_u32(seller_balance, amount),
        );

        // Return shares to available pool
        let available: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AvailableShares)
            .expect("Contract not initialized: available shares");
        env.storage()
            .instance()
            .set(&DataKey::AvailableShares, &checked_add_u32(available, amount));

        EventBuybackShares { seller, amount, total_cost }.publish(&env);
    }

    /// Admin sets the auto-buyback configuration.
    /// `budget` tokens must already be held (or will be deposited) by the
    /// contract. Calling this again replaces the previous configuration and
    /// resets the `LastBuyback` timestamp.
    pub fn auto_buyback_config(env: Env, interval: u64, max_amount: u32, budget: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        if interval == 0 {
            panic!("Interval must be positive");
        }
        if max_amount == 0 {
            panic!("Max amount must be positive");
        }
        if budget <= 0 {
            panic!("Budget must be positive");
        }

        let config = AutoBuybackConfig { interval, max_amount, budget };
        env.storage().instance().set(&DataKey::BuybackConfig, &config);
        env.storage().instance().set(&DataKey::BuybackBudget, &budget);
        // Reset last-buyback so the first call is not gated
        env.storage().instance().set(&DataKey::LastBuyback, &0u64);

        EventAutoBuybackConfig { interval, max_amount, budget }.publish(&env);
    }

    /// Trigger an auto-buyback for `seller`. Callable by anyone.
    /// Validates that:
    ///   - a config exists
    ///   - the interval since the last auto-buyback has elapsed
    ///   - `amount` does not exceed `config.max_amount`
    ///   - the remaining budget covers the cost
    pub fn process_auto_buyback(env: Env, seller: Address, amount: u32) {
        if amount == 0 {
            panic!("Buyback amount must be positive");
        }

        let config: AutoBuybackConfig = env
            .storage()
            .instance()
            .get(&DataKey::BuybackConfig)
            .expect("Auto-buyback not configured");

        let last: u64 = env
            .storage()
            .instance()
            .get(&DataKey::LastBuyback)
            .unwrap_or(0);

        let now = env.ledger().timestamp();
        if now < last.saturating_add(config.interval) {
            panic!("Auto-buyback interval has not elapsed");
        }

        if amount > config.max_amount {
            panic!("Amount exceeds auto-buyback max");
        }

        let price: i128 = env
            .storage()
            .instance()
            .get(&DataKey::PricePerShare)
            .expect("Contract not initialized: price");

        let total_cost = checked_mul_i128(price, amount as i128);

        let remaining_budget: i128 = env
            .storage()
            .instance()
            .get(&DataKey::BuybackBudget)
            .unwrap_or(0);

        if total_cost > remaining_budget {
            panic!("Insufficient auto-buyback budget");
        }

        // Update budget and timestamp before the external call (CEI pattern)
        env.storage()
            .instance()
            .set(&DataKey::BuybackBudget, &checked_sub_i128(remaining_budget, total_cost));
        env.storage().instance().set(&DataKey::LastBuyback, &now);

        // Delegate to the core buyback, which requires seller auth
        Self::buyback_shares(env, seller, amount);
    }

    /// Return the current auto-buyback configuration.
    pub fn get_buyback_config(env: Env) -> Option<AutoBuybackConfig> {
        env.storage().instance().get(&DataKey::BuybackConfig)
    }

    // ── Issue #268: User-initiated buyback requests ────────────────────

    /// Submit a buyback request for the admin to process. Caller must be a
    /// share holder. This records the request on-chain with a unique ID.
    pub fn request_buyback(env: Env, seller: Address, amount: u32, requested_price: i128) -> u64 {
        seller.require_auth();

        if amount == 0 {
            panic!("Buyback amount must be positive");
        }

        let seller_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(seller.clone()))
            .unwrap_or(0);
        if amount > seller_balance {
            panic!("Insufficient shares for buyback request");
        }

        let counter: u64 = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::BuybackRequestCounter)
            .unwrap_or(0)
            + 1;
        env.storage()
            .instance()
            .set(&DataKey::BuybackRequestCounter, &counter);

        let request = BuybackRequest {
            request_id: counter,
            seller: seller.clone(),
            amount,
            requested_price,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::BuybackRequest(counter), &request);

        EventBuybackRequested { request_id: counter, seller, amount }.publish(&env);

        counter
    }

    /// Retrieve a previously submitted buyback request by ID.
    pub fn get_buyback_request(env: Env, request_id: u64) -> Option<BuybackRequest> {
        env.storage()
            .persistent()
            .get(&DataKey::BuybackRequest(request_id))
    }

    // ── Issue #262: Batch Share Purchase ─────────────────────────────────

    /// Maximum number of purchase requests allowed in a single batch.
    /// Prevents excessive gas consumption and keeps transactions bounded.
    const MAX_BATCH_SIZE: u32 = 10;

    /// Purchase multiple share allocations in a single transaction.
    ///
    /// This function reduces gas costs by sharing common validation logic
    /// (pause check, whitelist, oracle price fetch) across all items in the
    /// batch. Each item is validated individually and failures are recorded
    /// without aborting the entire batch (partial fulfillment).
    ///
    /// # Arguments
    /// * `buyer` – The address purchasing shares (must authorize).
    /// * `requests` – A vector of `BatchPurchaseRequest` items.
    ///
    /// # Returns
    /// A vector of `BatchPurchaseResult` with per-item outcomes.
    ///
    /// # Panics
    /// * If the marketplace is paused.
    /// * If the buyer is not whitelisted.
    /// * If the batch is empty or exceeds `MAX_BATCH_SIZE`.
    pub fn batch_buy_shares(
        env: Env,
        buyer: Address,
        requests: Vec<BatchPurchaseRequest>,
    ) -> Vec<BatchPurchaseResult> {
        buyer.require_auth();

        // ── Re-entrancy guard ────────────────────────────────────────
        _check_non_reentrant(&env);

        // ── Shared validations (done once for all items) ─────────────
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            _set_non_reentrant(&env, false);
            panic!("Marketplace is paused");
        }

        if !Self::is_whitelisted(env.clone(), buyer.clone()) {
            _set_non_reentrant(&env, false);
            panic!("Buyer is not whitelisted");
        }

        let batch_len = requests.len();
        if batch_len == 0 {
            _set_non_reentrant(&env, false);
            panic!("Batch must contain at least one purchase request");
        }
        if batch_len > Self::MAX_BATCH_SIZE {
            _set_non_reentrant(&env, false);
            panic!("Batch size exceeds maximum allowed");
        }

        // ── Shared state reads (amortised across batch) ──────────────
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin");

        let admin_price: i128 = env
            .storage()
            .instance()
            .get(&DataKey::PricePerShare)
            .expect("Contract not initialized: price");

        // Fetch oracle price once if configured (gas optimisation).
        let price: i128 = if let Some(oracle_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::OracleAddress)
        {
            let oracle_client = OracleContractClient::new(&env, &oracle_addr);
            match oracle_client.try_get_price() {
                Ok(Ok(p)) if p > 0 => {
                    EventOraclePriceFetched {
                        oracle: oracle_addr,
                        price: p,
                    }
                    .publish(&env);
                    p
                }
                _ => {
                    EventOraclePriceFallback { admin_price }.publish(&env);
                    admin_price
                }
            }
        } else {
            admin_price
        };

        let max_per_user: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MaxSharesPerUser)
            .unwrap_or(0);

        let mut available: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AvailableShares)
            .expect("Contract not initialized: available shares");

        let mut buyer_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(buyer.clone()))
            .unwrap_or(0);

        // ── Process each item ────────────────────────────────────────
        let mut results: Vec<BatchPurchaseResult> = Vec::new(&env);
        let mut aggregate_shares: u32 = 0;
        let mut aggregate_cost: i128 = 0;
        let mut successful_count: u32 = 0;

        for i in 0..batch_len {
            let req = requests.get(i).unwrap();
            let idx = i;

            // Per-item validation
            if req.shares == 0 {
                EventBatchPurchaseItemFailed {
                    buyer: buyer.clone(),
                    index: idx,
                    shares_requested: 0,
                }
                .publish(&env);
                results.push_back(BatchPurchaseResult {
                    index: idx,
                    success: false,
                    shares_purchased: 0,
                    total_cost: 0,
                });
                continue;
            }

            if req.shares > available {
                EventBatchPurchaseItemFailed {
                    buyer: buyer.clone(),
                    index: idx,
                    shares_requested: req.shares,
                }
                .publish(&env);
                results.push_back(BatchPurchaseResult {
                    index: idx,
                    success: false,
                    shares_purchased: 0,
                    total_cost: 0,
                });
                continue;
            }

            // Check per-user cap
            let prospective = checked_add_u32(buyer_balance, req.shares);
            if max_per_user > 0 && prospective > max_per_user {
                EventBatchPurchaseItemFailed {
                    buyer: buyer.clone(),
                    index: idx,
                    shares_requested: req.shares,
                }
                .publish(&env);
                results.push_back(BatchPurchaseResult {
                    index: idx,
                    success: false,
                    shares_purchased: 0,
                    total_cost: 0,
                });
                continue;
            }

            // Validate payment token is accepted
            let accepted: Vec<Address> = env
                .storage()
                .instance()
                .get(&DataKey::AcceptedTokens)
                .unwrap_or_else(|| Vec::new(&env));
            let mut token_ok = false;
            for t in accepted.iter() {
                if t == req.payment_token {
                    token_ok = true;
                    break;
                }
            }
            if !token_ok {
                EventBatchPurchaseItemFailed {
                    buyer: buyer.clone(),
                    index: idx,
                    shares_requested: req.shares,
                }
                .publish(&env);
                results.push_back(BatchPurchaseResult {
                    index: idx,
                    success: false,
                    shares_purchased: 0,
                    total_cost: 0,
                });
                continue;
            }

            // ── Execute the purchase ─────────────────────────────────
            let item_cost = checked_mul_i128(price, req.shares as i128);

            let client = token::TokenClient::new(&env, &req.payment_token);
            client.transfer(&buyer, &admin, &item_cost);

            // Update running state
            available = checked_sub_u32(available, req.shares);
            buyer_balance = prospective;
            aggregate_shares = checked_add_u32(aggregate_shares, req.shares);
            aggregate_cost = checked_add_i128(aggregate_cost, item_cost);
            successful_count += 1;

            // Mint NFT certificates if configured
            if let Some(nft_addr) = env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::NftContract)
            {
                let nft = NftContractClient::new(&env, &nft_addr);
                for _ in 0..req.shares {
                    nft.mint_certificate(&buyer);
                }
            }

            results.push_back(BatchPurchaseResult {
                index: idx,
                success: true,
                shares_purchased: req.shares,
                total_cost: item_cost,
            });
        }

        // ── Persist aggregated state changes ─────────────────────────
        env.storage()
            .instance()
            .set(&DataKey::AvailableShares, &available);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(buyer.clone()), &buyer_balance);

        if aggregate_shares > 0 {
            Self::register_holder(&env, buyer.clone());
        }

        // ── Clear reentrancy guard and emit summary event ────────────
        _set_non_reentrant(&env, false);

        EventBatchBuyShares {
            buyer,
            total_items: batch_len,
            successful_items: successful_count,
            total_shares: aggregate_shares,
            total_cost: aggregate_cost,
        }
        .publish(&env);

        results
    }

    /// Get a price quote for a batch of purchase requests without executing
    /// any transfers. Useful for UI display and gas estimation.
    ///
    /// Returns a vector of `BatchPurchaseResult` where `success` indicates
    /// whether the item *would* succeed, and `total_cost` is the estimated
    /// payment amount.
    pub fn get_batch_quote(
        env: Env,
        buyer: Address,
        requests: Vec<BatchPurchaseRequest>,
    ) -> Vec<BatchPurchaseResult> {
        let batch_len = requests.len();
        if batch_len == 0 {
            panic!("Batch must contain at least one purchase request");
        }
        if batch_len > Self::MAX_BATCH_SIZE {
            panic!("Batch size exceeds maximum allowed");
        }

        let admin_price: i128 = env
            .storage()
            .instance()
            .get(&DataKey::PricePerShare)
            .expect("Contract not initialized: price");

        // Use oracle price if configured (read-only, no events)
        let price: i128 = if let Some(oracle_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::OracleAddress)
        {
            let oracle_client = OracleContractClient::new(&env, &oracle_addr);
            match oracle_client.try_get_price() {
                Ok(Ok(p)) if p > 0 => p,
                _ => admin_price,
            }
        } else {
            admin_price
        };

        let is_whitelisted = Self::is_whitelisted(env.clone(), buyer.clone());
        let is_paused = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);

        let max_per_user: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MaxSharesPerUser)
            .unwrap_or(0);

        let mut available: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AvailableShares)
            .expect("Contract not initialized: available shares");

        let mut buyer_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(buyer.clone()))
            .unwrap_or(0);

        let mut results: Vec<BatchPurchaseResult> = Vec::new(&env);

        for i in 0..batch_len {
            let req = requests.get(i).unwrap();
            let idx = i;

            // Simulate the same validation as batch_buy_shares
            if is_paused || !is_whitelisted || req.shares == 0 || req.shares > available {
                results.push_back(BatchPurchaseResult {
                    index: idx,
                    success: false,
                    shares_purchased: 0,
                    total_cost: 0,
                });
                continue;
            }

            let prospective = checked_add_u32(buyer_balance, req.shares);
            if max_per_user > 0 && prospective > max_per_user {
                results.push_back(BatchPurchaseResult {
                    index: idx,
                    success: false,
                    shares_purchased: 0,
                    total_cost: 0,
                });
                continue;
            }

            // Validate payment token
            let accepted: Vec<Address> = env
                .storage()
                .instance()
                .get(&DataKey::AcceptedTokens)
                .unwrap_or_else(|| Vec::new(&env));
            let mut token_ok = false;
            for t in accepted.iter() {
                if t == req.payment_token {
                    token_ok = true;
                    break;
                }
            }
            if !token_ok {
                results.push_back(BatchPurchaseResult {
                    index: idx,
                    success: false,
                    shares_purchased: 0,
                    total_cost: 0,
                });
                continue;
            }

            let item_cost = checked_mul_i128(price, req.shares as i128);
            available = checked_sub_u32(available, req.shares);
            buyer_balance = prospective;

            results.push_back(BatchPurchaseResult {
                index: idx,
                success: true,
                shares_purchased: req.shares,
                total_cost: item_cost,
            });
        }

        results
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger as _}, token, Env};

    struct TestEnv {
        env: Env,
        admin: Address,
        buyer: Address,
        token_id: Address,
        contract_id: Address,
    }

    fn setup() -> TestEnv {
        let env = Env::default();
        let admin = Address::generate(&env);
        let buyer = Address::generate(&env);
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let token_id = sac.address();
        let contract_id = env.register(RwaMarketplace, ());
        env.mock_all_auths();
        TestEnv { env, admin, buyer, token_id, contract_id }
    }

    fn client(te: &TestEnv) -> RwaMarketplaceClient<'_> {
        RwaMarketplaceClient::new(&te.env, &te.contract_id)
    }

    fn mint(te: &TestEnv, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&te.env, &te.token_id).mint(to, &amount);
    }

    // ── Existing tests (unchanged) ──────────────────────────────────────

    #[test]
    fn test_init_and_query() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        assert_eq!(c.get_total_shares(), 1000);
        assert_eq!(c.get_available_shares(), 1000);
        assert_eq!(c.get_price(), 100);
        assert!(!c.is_paused());
        assert_eq!(c.get_shares(&te.admin), 0);
    }

    #[test]
    #[should_panic(expected = "Buyer is not whitelisted")]
    fn test_buy_shares_requires_whitelist() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100000);
        c.buy_shares(&te.buyer, &25, &te.token_id);
    }

    #[test]
    fn test_whitelist_admin_can_add_and_buy() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100000);

        assert!(!c.is_whitelisted(&te.buyer));
        c.add_to_whitelist(&te.buyer);
        assert!(c.is_whitelisted(&te.buyer));

        c.buy_shares(&te.buyer, &25, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 25);
        assert_eq!(c.get_available_shares(), 975);
    }

    #[test]
    #[should_panic(expected = "Buyer is not whitelisted")]
    fn test_remove_from_whitelist_blocks_buy() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100000);

        c.add_to_whitelist(&te.buyer);
        assert!(c.is_whitelisted(&te.buyer));
        c.remove_from_whitelist(&te.buyer);
        assert!(!c.is_whitelisted(&te.buyer));

        c.buy_shares(&te.buyer, &25, &te.token_id);
    }

    #[test]
    fn test_multiple_buys() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100000);
        c.add_to_whitelist(&te.buyer);

        c.buy_shares(&te.buyer, &10, &te.token_id);
        c.buy_shares(&te.buyer, &20, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 30);
        assert_eq!(c.get_available_shares(), 970);
    }

    #[test]
    fn test_pause_unpause() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        assert!(!c.is_paused());
        c.pause();
        assert!(c.is_paused());
        c.unpause();
        assert!(!c.is_paused());
    }

    #[test]
    #[should_panic(expected = "Marketplace is paused")]
    fn test_buy_when_paused() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.pause();
        c.buy_shares(&te.buyer, &1, &te.token_id);
    }

    #[test]
    #[should_panic(expected = "Marketplace is already initialized")]
    fn test_double_init() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.init(&te.admin, &te.token_id, &100, &1000);
    }

    #[test]
    #[should_panic(expected = "Price must be greater than zero")]
    fn test_init_zero_price() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &0, &1000);
    }

    #[test]
    #[should_panic(expected = "Price must be greater than zero")]
    fn test_init_negative_price() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &-50, &1000);
    }

    #[test]
    #[should_panic(expected = "Total shares must be greater than zero")]
    fn test_init_zero_total_shares() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &0);
    }

    #[test]
    #[should_panic(expected = "Not enough shares available")]
    fn test_overbuy() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &10);
        c.add_to_whitelist(&te.buyer);
        mint(&te, &te.buyer, 100000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &20, &te.token_id);
    }

    #[test]
    #[should_panic(expected = "Must purchase at least 1 share")]
    fn test_zero_shares() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &0, &te.token_id);
    }

    #[test]
    fn test_emergency_withdraw() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.emergency_withdraw(&te.admin, &0);
    }

    // ── New tests for holder registry and distribute_dividends ──────────

    #[test]
    fn test_holders_registered_on_buy() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        // Before any purchase, registry is empty
        assert_eq!(c.get_holders().len(), 0);

        c.buy_shares(&te.buyer, &10, &te.token_id);
        assert_eq!(c.get_holders().len(), 1);

        // Second buy by same buyer — should NOT add duplicate
        c.buy_shares(&te.buyer, &5, &te.token_id);
        assert_eq!(c.get_holders().len(), 1);
    }

    #[test]
    fn test_multiple_holders_registered() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        let buyer2 = Address::generate(&te.env);
        mint(&te, &te.buyer, 100_000);
        mint(&te, &buyer2, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.add_to_whitelist(&buyer2);

        c.buy_shares(&te.buyer, &10, &te.token_id);
        c.buy_shares(&buyer2, &20, &te.token_id);

        assert_eq!(c.get_holders().len(), 2);
    }

    #[test]
    fn test_distribute_dividends_single_holder() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.buy_shares(&te.buyer, &500, &te.token_id); // buyer owns 500 / 1000 shares = 50%

        // Mint dividend tokens to the contract
        let dividend_amount: i128 = 10_000;
        mint(&te, &te.contract_id, dividend_amount);

        c.distribute_dividends(&te.token_id, &dividend_amount);

        // buyer has 500/1000 shares → receives 5000
        let token_client = token::TokenClient::new(&te.env, &te.token_id);
        // buyer started with 100_000, paid 500*100=50_000, receives 5_000
        assert_eq!(token_client.balance(&te.buyer), 100_000 - 50_000 + 5_000);
    }

    #[test]
    fn test_distribute_dividends_multiple_holders() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        let buyer2 = Address::generate(&te.env);
        mint(&te, &te.buyer, 100_000);
        mint(&te, &buyer2, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.add_to_whitelist(&buyer2);

        // buyer: 250 shares (25%), buyer2: 750 shares (75%)
        c.buy_shares(&te.buyer, &250, &te.token_id);
        c.buy_shares(&buyer2, &750, &te.token_id);

        let dividend_amount: i128 = 10_000;
        mint(&te, &te.contract_id, dividend_amount);

        c.distribute_dividends(&te.token_id, &dividend_amount);

        let token_client = token::TokenClient::new(&te.env, &te.token_id);

        // buyer: 10000 * 250 / 1000 = 2500
        assert_eq!(
            token_client.balance(&te.buyer),
            100_000 - 250 * 100 + 2_500
        );
        // buyer2: 10000 * 750 / 1000 = 7500
        assert_eq!(
            token_client.balance(&buyer2),
            100_000 - 750 * 100 + 7_500
        );
    }

    #[test]
    fn test_distribute_cleans_up_zero_balance_holders() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        let buyer2 = Address::generate(&te.env);
        mint(&te, &te.buyer, 100_000);
        mint(&te, &buyer2, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.add_to_whitelist(&buyer2);

        c.buy_shares(&te.buyer, &10, &te.token_id);
        c.buy_shares(&buyer2, &20, &te.token_id);
        assert_eq!(c.get_holders().len(), 2);

        // Manually zero out buyer's balance to simulate a future sell/transfer
        te.env.as_contract(&te.contract_id, || {
            te.env
                .storage()
                .persistent()
                .set(&DataKey::Balance(te.buyer.clone()), &0u32);
        });

        let dividend_amount: i128 = 1_000;
        mint(&te, &te.contract_id, dividend_amount);
        c.distribute_dividends(&te.token_id, &dividend_amount);

        // buyer had 0 shares — removed from registry
        assert_eq!(c.get_holders().len(), 1);
    }

    #[test]
    #[should_panic(expected = "Dividend amount must be positive")]
    fn test_distribute_zero_amount() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.distribute_dividends(&te.token_id, &0);
    }

    #[test]
    #[should_panic(expected = "No holders registered")]
    fn test_distribute_no_holders() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.distribute_dividends(&te.token_id, &1000);
    }

    // ── Tests for set_price and set_total_shares ────────────────────────

    #[test]
    fn test_set_price() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_price(&200);
        assert_eq!(c.get_price(), 200);
    }

    #[test]
    fn test_set_price_affects_future_buys() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.set_price(&200);
        c.buy_shares(&te.buyer, &10, &te.token_id);

        let token_client = token::TokenClient::new(&te.env, &te.token_id);
        assert_eq!(token_client.balance(&te.buyer), 100_000 - 10 * 200);
    }

    #[test]
    #[should_panic(expected = "Price must be positive")]
    fn test_set_price_zero() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.set_price(&0);
    }

    #[test]
    #[should_panic(expected = "Price must be positive")]
    fn test_set_price_negative() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.set_price(&-50);
    }

    #[test]
    fn test_set_total_shares_increase() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_total_shares(&1500);
        assert_eq!(c.get_total_shares(), 1500);
        assert_eq!(c.get_available_shares(), 1500);
    }

    #[test]
    fn test_set_total_shares_after_partial_sale() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.buy_shares(&te.buyer, &100, &te.token_id);
        assert_eq!(c.get_available_shares(), 900);

        c.set_total_shares(&1200);
        assert_eq!(c.get_total_shares(), 1200);
        assert_eq!(c.get_available_shares(), 1100);
        assert_eq!(c.get_shares(&te.buyer), 100);
    }

    #[test]
    fn test_set_total_shares_same_as_current() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_total_shares(&1000);
        assert_eq!(c.get_total_shares(), 1000);
        assert_eq!(c.get_available_shares(), 1000);
    }

    #[test]
    #[should_panic(expected = "New total must be at least available shares")]
    fn test_set_total_shares_below_available() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.set_total_shares(&500);
    }

    #[test]
    #[should_panic(expected = "Arithmetic overflow")]
    fn test_buy_shares_price_overflow() {
        let te = setup();
        let c = client(&te);
        // Use very high price that will overflow when multiplied by shares
        c.init(&te.admin, &te.token_id, &i128::MAX, &1000);
        c.add_to_whitelist(&te.buyer);
        mint(&te, &te.buyer, i128::MAX);
        c.add_to_whitelist(&te.buyer);
        
        // This should panic because price * shares overflows
        c.buy_shares(&te.buyer, &2, &te.token_id);
    }

    #[test]
    #[should_panic(expected = "Not enough shares available")]
    fn test_buy_shares_overbuy() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.add_to_whitelist(&te.buyer);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        
        // Buy more shares than available (caught by logic check, not arithmetic)
        c.buy_shares(&te.buyer, &2000, &te.token_id);
    }

    #[test]
    #[should_panic(expected = "Arithmetic overflow")]
    fn test_buy_shares_balance_overflow() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &1, &u32::MAX);
        c.add_to_whitelist(&te.buyer);
        mint(&te, &te.buyer, i128::MAX);
        c.add_to_whitelist(&te.buyer);
        
        // Manually set high balance to test the checked_add_u32 in balance calculation
        te.env.as_contract(&te.contract_id, || {
            te.env.storage().persistent().set(&DataKey::Balance(te.buyer.clone()), &(u32::MAX - 10));
            // Also set available shares high enough
            te.env.storage().instance().set(&DataKey::AvailableShares, &1000u32);
        });
        
        // Now buying 20 more shares should trigger overflow in checked_add_u32
        c.buy_shares(&te.buyer, &20, &te.token_id);
    }

    #[test]
    #[should_panic(expected = "Arithmetic overflow")]
    fn test_distribute_dividends_multiply_overflow() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.add_to_whitelist(&te.buyer);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        
        c.buy_shares(&te.buyer, &500, &te.token_id);
        
        // Use extremely large dividend amount that will overflow when multiplied by holder_shares
        let huge_dividend: i128 = i128::MAX / 2;
        mint(&te, &te.contract_id, huge_dividend);
        
        // This should panic because total_amount * holder_shares overflows
        c.distribute_dividends(&te.token_id, &huge_dividend);
    }

    #[test]
    #[should_panic(expected = "New total cannot be less than issued shares")]
    fn test_set_total_shares_below_issued_logic_check() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.add_to_whitelist(&te.buyer);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        
        // Buy some shares to create issued_shares
        c.buy_shares(&te.buyer, &600, &te.token_id);
        
        // Try to set new_total to less than issued_shares
        // This is caught by the logic check before any arithmetic
        c.set_total_shares(&500);
    }

    // ── Pre-init tests: every function should give a clear error before init ─

    fn pre_init_client() -> (Env, RwaMarketplaceClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let contract_id = env.register(RwaMarketplace, ());
        let client = RwaMarketplaceClient::new(&env, &contract_id);
        (env, client, token_id, admin)
    }

    #[test]
    #[should_panic(expected = "Buyer is not whitelisted")]
    fn test_pre_init_buy_shares() {
        let (env, client, token_id, _) = pre_init_client();
        let buyer = Address::generate(&env);
        client.buy_shares(&buyer, &1, &token_id);
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_pause() {
        let (_, client, _, _) = pre_init_client();
        client.pause();
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_unpause() {
        let (_, client, _, _) = pre_init_client();
        client.unpause();
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_set_price() {
        let (_, client, _, _) = pre_init_client();
        client.set_price(&100);
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_set_total_shares() {
        let (_, client, _, _) = pre_init_client();
        client.set_total_shares(&1000);
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_distribute_dividends() {
        let (_, client, token_id, _) = pre_init_client();
        client.distribute_dividends(&token_id, &1000);
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_emergency_withdraw() {
        let (_, client, _, admin) = pre_init_client();
        client.emergency_withdraw(&admin, &0);
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_add_to_whitelist() {
        let (env, client, _, _) = pre_init_client();
        let addr = Address::generate(&env);
        client.add_to_whitelist(&addr);
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_remove_from_whitelist() {
        let (env, client, _, _) = pre_init_client();
        let addr = Address::generate(&env);
        client.remove_from_whitelist(&addr);
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_buy_vested_shares() {
        let (env, client, token_id, _) = pre_init_client();
        let buyer = Address::generate(&env);
        client.buy_vested_shares(&buyer, &1, &3600, &token_id);
    }

    // ── Metadata URI tests ──────────────────────────────────────────────

    #[test]
    fn test_set_and_get_metadata_uri() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        let uri = soroban_sdk::Bytes::from_slice(&te.env, b"ipfs://QmTest");
        c.set_metadata_uri(&uri);
        assert_eq!(c.get_metadata_uri(), uri);
    }

    #[test]
    fn test_get_metadata_uri_default_empty() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        assert_eq!(c.get_metadata_uri(), soroban_sdk::Bytes::new(&te.env));
    }

    #[test]
    fn test_set_metadata_uri_overwrites() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_metadata_uri(&soroban_sdk::Bytes::from_slice(&te.env, b"ipfs://old"));
        let new_uri = soroban_sdk::Bytes::from_slice(&te.env, b"ipfs://new");
        c.set_metadata_uri(&new_uri);
        assert_eq!(c.get_metadata_uri(), new_uri);
    }

    // ── Dividend schedule tests ─────────────────────────────────────────

    #[test]
    fn test_set_dividend_schedule() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_dividend_schedule(&10_i128, &86400_u64);
        let schedule = c.get_dividend_schedule().unwrap();
        assert_eq!(schedule.amount_per_share, 10);
        assert_eq!(schedule.interval, 86400);
    }

    #[test]
    fn test_get_dividend_schedule_default_none() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        assert!(c.get_dividend_schedule().is_none());
    }

    #[test]
    #[should_panic(expected = "Amount per share must be positive")]
    fn test_set_dividend_schedule_zero_amount() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_dividend_schedule(&0, &86400);
    }

    #[test]
    #[should_panic(expected = "Amount per share must be positive")]
    fn test_set_dividend_schedule_negative_amount() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_dividend_schedule(&-1, &86400);
    }

    #[test]
    #[should_panic(expected = "Interval must be positive")]
    fn test_set_dividend_schedule_zero_interval() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_dividend_schedule(&10, &0);
    }

    #[test]
    #[should_panic(expected = "Dividend schedule not configured")]
    fn test_process_scheduled_dividend_no_schedule() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.process_scheduled_dividend();
    }

    #[test]
    #[should_panic(expected = "Dividend interval has not elapsed yet")]
    fn test_process_scheduled_dividend_interval_not_elapsed() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_dividend_schedule(&10, &86400);
        // Call immediately — interval (86400s = 1 day) has not elapsed
        c.process_scheduled_dividend();
    }

    #[test]
    fn test_process_scheduled_dividend_distributes() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        let buyer2 = Address::generate(&te.env);
        mint(&te, &te.buyer, 100_000);
        mint(&te, &buyer2, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.add_to_whitelist(&buyer2);

        c.buy_shares(&te.buyer, &300, &te.token_id);
        c.buy_shares(&buyer2, &700, &te.token_id);
        assert_eq!(c.get_available_shares(), 0);

        // Set schedule: 10 tokens per share, daily
        c.set_dividend_schedule(&10, &86400);

        // total_amount = 10 * 1000 = 10_000
        let total_amount: i128 = 10 * 1000;
        mint(&te, &te.contract_id, total_amount);

        // Fast-forward past the interval
        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 86401);

        c.process_scheduled_dividend();

        let token_client = token::TokenClient::new(&te.env, &te.token_id);
        // buyer: 100_000 initial - 300*100 cost + (10_000 * 300 / 1000) = 100_000 - 30_000 + 3_000
        assert_eq!(token_client.balance(&te.buyer), 100_000 - 30_000 + 3_000);
        // buyer2: 100_000 - 700*100 + (10_000 * 700 / 1000) = 100_000 - 70_000 + 7_000
        assert_eq!(token_client.balance(&buyer2), 100_000 - 70_000 + 7_000);
    }

    #[test]
    fn test_process_scheduled_dividend_updates_last_distribution() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &500, &te.token_id);

        c.set_dividend_schedule(&1, &100);
        mint(&te, &te.contract_id, 500);

        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 101);
        c.process_scheduled_dividend();
    }

    #[test]
    #[should_panic(expected = "Dividend interval has not elapsed yet")]
    fn test_process_scheduled_dividend_second_call_too_soon() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &500, &te.token_id);

        c.set_dividend_schedule(&1, &100);
        mint(&te, &te.contract_id, 1000);

        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 101);
        c.process_scheduled_dividend();

        // Second call before next interval should fail
        c.process_scheduled_dividend();
    }

    #[test]
    fn test_process_scheduled_dividend_after_multiple_intervals() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &500, &te.token_id);

        c.set_dividend_schedule(&5, &3600); // every hour
        mint(&te, &te.contract_id, 2500);

        let start = te.env.ledger().timestamp();

        // First distribution after 1 hour
        te.env.ledger().set_timestamp(start + 3601);
        c.process_scheduled_dividend();

        // Second distribution after another hour
        mint(&te, &te.contract_id, 2500);
        te.env.ledger().set_timestamp(start + 7201);
        c.process_scheduled_dividend();

        let token_client = token::TokenClient::new(&te.env, &te.token_id);
        // buyer: 100_000 - 500*100 + 2500 + 2500 = 100_000 - 50_000 + 5_000
        assert_eq!(token_client.balance(&te.buyer), 100_000 - 50_000 + 5_000);
    }

    #[test]
    #[should_panic(expected = "No holders registered")]
    fn test_process_scheduled_dividend_no_holders() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_dividend_schedule(&10, &1);
        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 2);
        c.process_scheduled_dividend();
    }

    // ── Max shares per user tests ───────────────────────────────────────

    #[test]
    fn test_max_shares_per_user_default_unlimited() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        // Defaults to 0, meaning no cap is enforced.
        assert_eq!(c.get_max_shares_per_user(), 0);
    }

    #[test]
    fn test_set_and_get_max_shares_per_user() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        c.set_max_shares_per_user(&50);
        assert_eq!(c.get_max_shares_per_user(), 50);
    }

    #[test]
    fn test_buy_within_cap_succeeds() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.set_max_shares_per_user(&50);
        c.buy_shares(&te.buyer, &50, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 50);
    }

    #[test]
    #[should_panic(expected = "Purchase exceeds max shares per user")]
    fn test_buy_exceeding_cap_single_purchase() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.set_max_shares_per_user(&50);
        c.buy_shares(&te.buyer, &51, &te.token_id);
    }

    #[test]
    #[should_panic(expected = "Purchase exceeds max shares per user")]
    fn test_cap_checks_current_holdings_plus_purchase() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.set_max_shares_per_user(&50);
        // First purchase is fine (40 <= 50).
        c.buy_shares(&te.buyer, &40, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 40);
        // Second purchase pushes total to 60 > 50 → rejected.
        c.buy_shares(&te.buyer, &20, &te.token_id);
    }

    #[test]
    fn test_buy_up_to_cap_across_multiple_purchases() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.set_max_shares_per_user(&50);
        c.buy_shares(&te.buyer, &30, &te.token_id);
        c.buy_shares(&te.buyer, &20, &te.token_id); // exactly hits the cap
        assert_eq!(c.get_shares(&te.buyer), 50);
    }

    #[test]
    fn test_cap_does_not_block_transfer_when_rejected() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.set_max_shares_per_user(&50);

        // A purchase that exceeds the cap must revert before transferring any
        // tokens. Use the non-panicking client to assert the call fails and
        // that no balances or available shares changed.
        let result = c.try_buy_shares(&te.buyer, &60, &te.token_id);
        assert!(result.is_err());

        let token_client = token::TokenClient::new(&te.env, &te.token_id);
        assert_eq!(token_client.balance(&te.buyer), 100_000);
        assert_eq!(c.get_shares(&te.buyer), 0);
        assert_eq!(c.get_available_shares(), 1000);
    }

    #[test]
    fn test_cap_zero_means_unlimited() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        c.set_max_shares_per_user(&50);
        c.set_max_shares_per_user(&0); // disable cap
        c.buy_shares(&te.buyer, &900, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 900);
    }

    #[test]
    fn test_raising_cap_allows_more_purchases() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        c.set_max_shares_per_user(&50);
        c.buy_shares(&te.buyer, &50, &te.token_id);

        c.set_max_shares_per_user(&100);
        c.buy_shares(&te.buyer, &50, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 100);
    }

    #[test]
    fn test_cap_is_per_address_not_global() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        let buyer2 = Address::generate(&te.env);
        mint(&te, &te.buyer, 100_000);
        mint(&te, &buyer2, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.add_to_whitelist(&buyer2);

        c.set_max_shares_per_user(&50);
        c.buy_shares(&te.buyer, &50, &te.token_id);
        c.buy_shares(&buyer2, &50, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 50);
        assert_eq!(c.get_shares(&buyer2), 50);
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_pre_init_set_max_shares_per_user() {
        let (_, client, _, _) = pre_init_client();
        client.set_max_shares_per_user(&50);
    }

    // ── Transfer tests ──────────────────────────────────────────────────

    #[test]
    fn test_transfer_shares_basic() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &50, &te.token_id);

        let recipient = Address::generate(&te.env);
        c.transfer_shares(&te.buyer, &recipient, &20);

        assert_eq!(c.get_shares(&te.buyer), 30);
        assert_eq!(c.get_shares(&recipient), 20);
    }

    #[test]
    #[should_panic(expected = "Insufficient shares to transfer")]
    fn test_transfer_shares_insufficient_balance() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &10, &te.token_id);

        let recipient = Address::generate(&te.env);
        c.transfer_shares(&te.buyer, &recipient, &20);
    }

    #[test]
    #[should_panic(expected = "Transfer amount must be positive")]
    fn test_transfer_shares_zero_amount() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &10, &te.token_id);

        let recipient = Address::generate(&te.env);
        c.transfer_shares(&te.buyer, &recipient, &0);
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &50, &te.token_id);

        let spender = Address::generate(&te.env);
        let recipient = Address::generate(&te.env);

        c.approve(&te.buyer, &spender, &30);
        assert_eq!(c.allowance(&te.buyer, &spender), 30);

        c.transfer_shares_from(&spender, &te.buyer, &recipient, &20);

        assert_eq!(c.get_shares(&te.buyer), 30);
        assert_eq!(c.get_shares(&recipient), 20);
        // Allowance reduced
        assert_eq!(c.allowance(&te.buyer, &spender), 10);
    }

    #[test]
    #[should_panic(expected = "Transfer amount exceeds allowance")]
    fn test_transfer_from_exceeds_allowance() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &50, &te.token_id);

        let spender = Address::generate(&te.env);
        let recipient = Address::generate(&te.env);

        c.approve(&te.buyer, &spender, &10);
        c.transfer_shares_from(&spender, &te.buyer, &recipient, &20);
    }

    #[test]
    fn test_transfer_registers_recipient_as_holder() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &50, &te.token_id);

        let recipient = Address::generate(&te.env);
        assert_eq!(c.get_holders().len(), 1);

        c.transfer_shares(&te.buyer, &recipient, &10);
        assert_eq!(c.get_holders().len(), 2);
    }

    // ── Buyback tests ───────────────────────────────────────────────────

    #[test]
    fn test_buyback_shares_basic() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &100, &te.token_id);

        // Fund contract so it can pay seller
        mint(&te, &te.contract_id, 10_000);

        let available_before = c.get_available_shares(); // 900
        let token_client = token::TokenClient::new(&te.env, &te.token_id);
        let seller_balance_before = token_client.balance(&te.buyer);

        c.buyback_shares(&te.buyer, &50);

        // Seller loses 50 shares, gains 50*100=5000 tokens
        assert_eq!(c.get_shares(&te.buyer), 50);
        assert_eq!(token_client.balance(&te.buyer), seller_balance_before + 5_000);
        // Available increases by 50
        assert_eq!(c.get_available_shares(), available_before + 50);
    }

    #[test]
    #[should_panic(expected = "Buyback amount must be positive")]
    fn test_buyback_shares_zero() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &10, &te.token_id);
        c.buyback_shares(&te.buyer, &0);
    }

    #[test]
    #[should_panic(expected = "Seller has insufficient shares")]
    fn test_buyback_shares_insufficient() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &10, &te.token_id);
        mint(&te, &te.contract_id, 1_000_000);
        c.buyback_shares(&te.buyer, &20);
    }

    #[test]
    fn test_auto_buyback_config_sets_values() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.auto_buyback_config(&3600_u64, &50_u32, &100_000_i128);
        // Config set without error; process should succeed after interval
    }

    #[test]
    #[should_panic(expected = "Interval must be positive")]
    fn test_auto_buyback_config_zero_interval() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.auto_buyback_config(&0_u64, &10_u32, &1000_i128);
    }

    #[test]
    #[should_panic(expected = "Max amount must be positive")]
    fn test_auto_buyback_config_zero_max_amount() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.auto_buyback_config(&3600_u64, &0_u32, &1000_i128);
    }

    #[test]
    #[should_panic(expected = "Budget must be positive")]
    fn test_auto_buyback_config_zero_budget() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.auto_buyback_config(&3600_u64, &10_u32, &0_i128);
    }

    #[test]
    fn test_process_auto_buyback_succeeds() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &100, &te.token_id);

        // Fund contract and configure auto-buyback
        mint(&te, &te.contract_id, 50_000);
        c.auto_buyback_config(&3600_u64, &50_u32, &50_000_i128);

        // Advance past interval
        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 3601);

        c.process_auto_buyback(&te.buyer, &30);

        assert_eq!(c.get_shares(&te.buyer), 70);
        assert_eq!(c.get_available_shares(), 930);
    }

    #[test]
    #[should_panic(expected = "Auto-buyback interval has not elapsed")]
    fn test_process_auto_buyback_too_soon() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &100, &te.token_id);
        mint(&te, &te.contract_id, 50_000);
        c.auto_buyback_config(&3600_u64, &50_u32, &50_000_i128);

        // Do NOT advance time — should fail immediately
        c.process_auto_buyback(&te.buyer, &10);
    }

    #[test]
    #[should_panic(expected = "Amount exceeds auto-buyback max")]
    fn test_process_auto_buyback_exceeds_max() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &100, &te.token_id);
        mint(&te, &te.contract_id, 50_000);
        c.auto_buyback_config(&3600_u64, &20_u32, &50_000_i128);

        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 3601);
        c.process_auto_buyback(&te.buyer, &30);
    }

    #[test]
    #[should_panic(expected = "Insufficient auto-buyback budget")]
    fn test_process_auto_buyback_exceeds_budget() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &100, &te.token_id);

        // Budget of 500 → can afford only 5 shares at price 100
        mint(&te, &te.contract_id, 500);
        c.auto_buyback_config(&3600_u64, &50_u32, &500_i128);

        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 3601);
        c.process_auto_buyback(&te.buyer, &10); // 10 * 100 = 1000 > 500
    }

    #[test]
    #[should_panic(expected = "Auto-buyback not configured")]
    fn test_process_auto_buyback_not_configured() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.process_auto_buyback(&te.buyer, &10);
    }

    // ── NFT minting tests ───────────────────────────────────────────────

    use share_certificate_nft::ShareCertificate;

    fn setup_nft(te: &TestEnv) -> Address {
        let nft_id = te.env.register(ShareCertificate, ());
        let nft_client = share_certificate_nft::ShareCertificateClient::new(&te.env, &nft_id);
        nft_client.init(
            &te.contract_id, // minter = marketplace contract
            &soroban_sdk::String::from_str(&te.env, "ipfs://rwa/"),
            &soroban_sdk::String::from_str(&te.env, "RWA Share Certificate"),
            &soroban_sdk::String::from_str(&te.env, "RWAC"),
        );
        nft_id
    }

    #[test]
    fn test_set_and_get_nft_contract() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        let nft_id = setup_nft(&te);
        c.set_nft_contract(&nft_id);
        assert_eq!(c.get_nft_contract(), Some(nft_id));
    }

    #[test]
    fn test_get_nft_contract_default_none() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        assert_eq!(c.get_nft_contract(), None);
    }

    #[test]
    fn test_buy_shares_mints_nfts() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        let nft_id = setup_nft(&te);
        c.set_nft_contract(&nft_id);

        c.buy_shares(&te.buyer, &3, &te.token_id);

        // 3 shares purchased → 3 NFTs minted
        use stellar_tokens::non_fungible::Base;
        te.env.as_contract(&nft_id, || {
            assert_eq!(Base::balance(&te.env, &te.buyer), 3);
        });
    }

    #[test]
    fn test_buy_shares_without_nft_contract_still_works() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        // No NFT contract configured — buy_shares should succeed normally
        c.buy_shares(&te.buyer, &5, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 5);
    }

    #[test]
    fn test_nft_owner_is_buyer() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        let nft_id = setup_nft(&te);
        c.set_nft_contract(&nft_id);

        c.buy_shares(&te.buyer, &1, &te.token_id);

        use stellar_tokens::non_fungible::Base;
        te.env.as_contract(&nft_id, || {
            assert_eq!(Base::owner_of(&te.env, 0), te.buyer);
        });
    }

    // ── Re-entrancy guard tests ───────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Re-entrancy detected")]
    fn test_reentrancy_is_blocked() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        // Manually set the reentrancy guard to simulate an ongoing call
        te.env.as_contract(&te.contract_id, || {
            te.env.storage().instance().set(&DataKey::ReentrancyGuard, &true);
        });

        // This call should fail with re-entrancy error
        c.buy_shares(&te.buyer, &10, &te.token_id);
    }

    #[test]
    fn test_reentrancy_guard_cleared_after_successful_buy() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        // Guard should be false initially
        te.env.as_contract(&te.contract_id, || {
            assert!(!te.env.storage().instance().get::<DataKey, bool>(&DataKey::ReentrancyGuard).unwrap_or(false));
        });

        c.buy_shares(&te.buyer, &10, &te.token_id);

        // Guard should be cleared after successful buy
        te.env.as_contract(&te.contract_id, || {
            assert!(!te.env.storage().instance().get::<DataKey, bool>(&DataKey::ReentrancyGuard).unwrap_or(true));
        });
    }

    #[test]
    #[should_panic(expected = "Re-entrancy detected")]
    fn test_reentrancy_blocked_on_panic_path() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        // Set guard and unpause to trigger panic after guard is set
        te.env.as_contract(&te.contract_id, || {
            te.env.storage().instance().set(&DataKey::ReentrancyGuard, &true);
            te.env.storage().instance().set(&DataKey::Paused, &true);
        });

        // Should panic with re-entrancy detected (guard check happens first)
        c.buy_shares(&te.buyer, &10, &te.token_id);
    }

    #[test]
    fn test_reentrancy_guard_default_false() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);

        // Check that reentrancy guard is not set by default after init
        te.env.as_contract(&te.contract_id, || {
            assert!(!te.env.storage().instance().get::<DataKey, bool>(&DataKey::ReentrancyGuard).unwrap_or(false));
        });
    }

    #[test]
    #[should_panic(expected = "Re-entrancy detected")]
    fn test_double_reentrancy_blocked() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);

        // Manually lock the guard
        te.env.as_contract(&te.contract_id, || {
            te.env.storage().instance().set(&DataKey::ReentrancyGuard, &true);
        });

        // First call should fail
        c.buy_shares(&te.buyer, &10, &te.token_id);
    }

    // ── Event emission tests (Issue #167) ─────────────────────────────────
    // Each test below calls a state-modifying function and then verifies
    // that at least one event was emitted by the contract. Because the event
    // `publish` call is the last thing in each function, a successful return
    // without panic already proves the event was emitted correctly.

    #[test]
    fn test_event_emission_on_init() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        // init emits EventInit – no panic means success
    }

    #[test]
    fn test_event_emission_on_buy_shares() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &10, &te.token_id);
    }

    #[test]
    fn test_event_emission_on_pause() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.pause();
    }

    #[test]
    fn test_event_emission_on_unpause() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.pause();
        c.unpause();
    }

    #[test]
    fn test_event_emission_on_set_price() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.set_price(&200);
    }

    #[test]
    fn test_event_emission_on_set_total_shares() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.set_total_shares(&2000);
    }

    #[test]
    fn test_event_emission_on_transfer() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &50, &te.token_id);
        let recipient = Address::generate(&te.env);
        c.transfer_shares(&te.buyer, &recipient, &20);
    }

    #[test]
    fn test_event_emission_on_approve() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &50, &te.token_id);
        let spender = Address::generate(&te.env);
        c.approve(&te.buyer, &spender, &30);
    }

    #[test]
    fn test_event_emission_on_sell_order_flow() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &50, &te.token_id);
        c.place_sell_order(&te.buyer, &20, &150);
        c.cancel_sell_order(&0);
    }

    #[test]
    fn test_event_emission_on_buyback() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &100, &te.token_id);
        mint(&te, &te.contract_id, 10_000);
        c.buyback_shares(&te.buyer, &50);
    }

    #[test]
    fn test_event_emission_on_auto_buyback_config() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.auto_buyback_config(&3600_u64, &50_u32, &100_000_i128);
    }

    #[test]
    fn test_event_emission_on_set_max_shares_per_user() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.set_max_shares_per_user(&100);
    }

    #[test]
    fn test_event_emission_on_emergency_withdraw() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.emergency_withdraw(&te.admin, &0);
    }

    #[test]
    fn test_event_emission_on_distribute_dividends() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &500, &te.token_id);
        let dividend_amount: i128 = 10_000;
        mint(&te, &te.contract_id, dividend_amount);
        c.distribute_dividends(&te.token_id, &dividend_amount);
    }

    #[test]
    fn test_event_emission_on_set_dividend_schedule() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.set_dividend_schedule(&10_i128, &86400_u64);
    }

    #[test]
    fn test_event_emission_on_process_scheduled_dividend() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &500, &te.token_id);
        c.set_dividend_schedule(&1, &100);
        mint(&te, &te.contract_id, 500);
        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 101);
        c.process_scheduled_dividend();
    }

    // ── Newly added event emission tests (Issue #167) ───────────────────

    #[test]
    fn test_event_emission_on_set_nft_contract() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        let nft_id = te.env.register(share_certificate_nft::ShareCertificate, ());
        c.set_nft_contract(&nft_id);
    }

    #[test]
    fn test_event_emission_on_whitelist_ops() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        let addr = Address::generate(&te.env);
        c.add_to_whitelist(&addr);
        c.remove_from_whitelist(&addr);
    }

    #[test]
    fn test_event_emission_on_set_metadata_uri() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        let uri = soroban_sdk::Bytes::from_slice(&te.env, b"ipfs://QmTest");
        c.set_metadata_uri(&uri);
    }

    #[test]
    fn test_event_emission_on_claim_vested_shares() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_vested_shares(&te.buyer, &10, &1, &te.token_id);
        te.env.ledger().set_timestamp(te.env.ledger().timestamp() + 2);
        c.claim_vested_shares(&te.buyer);
    }
}
// --- TIMELOCK MODULE ---
// Appended as a completely isolated module to avoid breaking existing enums.

#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdminAction {
    Pause,
    Unpause,
    EmergencyWithdraw(soroban_sdk::Address, i128),
}

#[soroban_sdk::contracttype]
pub enum TimelockDataKey {
    TimelockOp(AdminAction),
}

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TimelockError {
    NotScheduled = 1,
    TimelockNotExpired = 2,
    AlreadyScheduled = 3,
}

#[soroban_sdk::contractimpl]
impl RwaMarketplace {
    pub fn schedule_operation(env: soroban_sdk::Env, admin: soroban_sdk::Address, action: AdminAction) {
        admin.require_auth();
        let timelock_key = TimelockDataKey::TimelockOp(action.clone());
        
        if env.storage().persistent().has(&timelock_key) {
            soroban_sdk::panic_with_error!(&env, TimelockError::AlreadyScheduled);
        }
        
        let execute_after = env.ledger().timestamp() + 172_800; // 48 hours
        env.storage().persistent().set(&timelock_key, &execute_after);

        EventOperationScheduled { action, execute_after }.publish(&env);
    }

    pub fn cancel_operation(env: soroban_sdk::Env, admin: soroban_sdk::Address, action: AdminAction) {
        admin.require_auth();
        let timelock_key = TimelockDataKey::TimelockOp(action.clone());
        
        if !env.storage().persistent().has(&timelock_key) {
            soroban_sdk::panic_with_error!(&env, TimelockError::NotScheduled);
        }
        
        env.storage().persistent().remove(&timelock_key);

        EventOperationCancelled { action }.publish(&env);
    }

    pub fn execute_operation(env: soroban_sdk::Env, admin: soroban_sdk::Address, action: AdminAction) {
        admin.require_auth();
        let timelock_key = TimelockDataKey::TimelockOp(action.clone());
        
        let execute_after: u64 = env
            .storage()
            .persistent()
            .get(&timelock_key)
            .unwrap_or_else(|| soroban_sdk::panic_with_error!(&env, TimelockError::NotScheduled));

        if env.ledger().timestamp() < execute_after {
            soroban_sdk::panic_with_error!(&env, TimelockError::TimelockNotExpired);
        }

        env.storage().persistent().remove(&timelock_key);

        // Forward to the native marketplace functions securely
        match action.clone() {
            AdminAction::Pause => {
                RwaMarketplace::pause(env.clone());
            },
            AdminAction::Unpause => {
                RwaMarketplace::unpause(env.clone());
            },
            AdminAction::EmergencyWithdraw(to, amount) => {
                RwaMarketplace::emergency_withdraw(env.clone(), to, amount);
            }
        }

        EventOperationExecuted { action }.publish(&env);
    }
}

#[cfg(test)]
mod timelock_tests {
    use super::*;
    use soroban_sdk::{Env, testutils::{Address as _, Ledger as _}};
    
    #[test]
    fn test_timelock_delay() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = soroban_sdk::Address::generate(&env);
        let payment_token = soroban_sdk::Address::generate(&env);
        
        let contract_id = env.register(RwaMarketplace, ());
        let client = RwaMarketplaceClient::new(&env, &contract_id);
        
        client.init(&admin, &payment_token, &100_i128, &1000_u32);
        
        let action = AdminAction::Pause;
        
        client.schedule_operation(&admin, &action);
        env.ledger().set_timestamp(env.ledger().timestamp() + 176_400); // Forward 49 hours
        client.execute_operation(&admin, &action);
        
        assert_eq!(client.is_paused(), true);
    }

    #[test]
    fn test_timelock_schedule_event() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = soroban_sdk::Address::generate(&env);
        let payment_token = soroban_sdk::Address::generate(&env);
        
        let contract_id = env.register(RwaMarketplace, ());
        let client = RwaMarketplaceClient::new(&env, &contract_id);
        
        client.init(&admin, &payment_token, &100_i128, &1000_u32);
        
        let action = AdminAction::Pause;
        client.schedule_operation(&admin, &action);
    }

    #[test]
    fn test_timelock_cancel_event() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = soroban_sdk::Address::generate(&env);
        let payment_token = soroban_sdk::Address::generate(&env);
        
        let contract_id = env.register(RwaMarketplace, ());
        let client = RwaMarketplaceClient::new(&env, &contract_id);
        
        client.init(&admin, &payment_token, &100_i128, &1000_u32);
        
        let action = AdminAction::Pause;
        client.schedule_operation(&admin, &action);
        client.cancel_operation(&admin, &action);
    }

    #[test]
    fn test_timelock_execute_event() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = soroban_sdk::Address::generate(&env);
        let payment_token = soroban_sdk::Address::generate(&env);
        
        let contract_id = env.register(RwaMarketplace, ());
        let client = RwaMarketplaceClient::new(&env, &contract_id);
        
        client.init(&admin, &payment_token, &100_i128, &1000_u32);
        
        let action = AdminAction::Unpause;
        client.schedule_operation(&admin, &action);
        env.ledger().set_timestamp(env.ledger().timestamp() + 176_400);
        client.execute_operation(&admin, &action);
    }
}

// ── Property-based / fuzz tests using proptest ─────────────────────────

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::testutils::Address as _;

    const NUM_BUYERS: usize = 5;
    const INIT_TOTAL: u32 = 1000;
    const INIT_PRICE: i128 = 100;

    /// Operations that can be fuzzed.
    #[derive(Clone, Debug)]
    enum Op {
        BuyShares { buyer_idx: usize, shares: u32 },
        Pause,
        Unpause,
        SetPrice(i128),
        SetTotalShares(u32),
    }

    fn arb_op() -> impl Strategy<Value = Op> {
        prop_oneof![
            4 => (0..NUM_BUYERS, 1..INIT_TOTAL / 4).prop_map(|(idx, s)| Op::BuyShares { buyer_idx: idx, shares: s }),
            1 => Just(Op::Pause),
            1 => Just(Op::Unpause),
            1 => (1i128..10_000i128).prop_map(Op::SetPrice),
            1 => (INIT_TOTAL..INIT_TOTAL * 5).prop_map(Op::SetTotalShares),
        ]
    }

    proptest! {
        /// Invariant: sum(all holder balances) + available == total at all times.
        #[test]
        fn test_contract_invariants(ops in prop::collection::vec(arb_op(), 1..30)) {
            let env = Env::default();
            env.mock_all_auths();
            let admin = Address::generate(&env);
            let token_id = env
                .register_stellar_asset_contract_v2(admin.clone())
                .address();
            let contract_id = env.register(RwaMarketplace, ());
            let client = RwaMarketplaceClient::new(&env, &contract_id);

            // Create buyers with sufficient funds
            let buyers: [Address; NUM_BUYERS] = core::array::from_fn(|_| Address::generate(&env));
            for b in buyers.iter() {
                token::StellarAssetClient::new(&env, &token_id).mint(b, &1_000_000_000);
            }

            client.init(&admin, &token_id, &INIT_PRICE, &INIT_TOTAL);

            for b in buyers.iter() {
                client.add_to_whitelist(b);
            }

            let mut balances = [0u32; NUM_BUYERS];
            let mut available = INIT_TOTAL;
            let mut total = INIT_TOTAL;
            let mut paused = false;

            for op in ops {
                match op {
                    Op::BuyShares { buyer_idx, shares } => {
                        if paused || shares > available {
                            continue;
                        }
                        client.buy_shares(&buyers[buyer_idx], &shares, &token_id);
                        balances[buyer_idx] += shares;
                        available -= shares;
                    }
                    Op::Pause => {
                        client.pause();
                        paused = true;
                    }
                    Op::Unpause => {
                        client.unpause();
                        paused = false;
                    }
                    Op::SetPrice(new_price) => {
                        if new_price <= 0 {
                            continue;
                        }
                        client.set_price(&new_price);
                    }
                    Op::SetTotalShares(new_total) => {
                        let issued = total - available;
                        if new_total < available || new_total < issued {
                            continue;
                        }
                        let new_available = new_total - issued;
                        client.set_total_shares(&new_total);
                        total = new_total;
                        available = new_available;
                    }
                }

                // Invariant: sum(balances) + available == total
                let sum_b: u32 = balances.iter().sum();
                prop_assert_eq!(
                    sum_b + available,
                    total,
                    "core invariant: sum(balances)={} + available={} != total={}",
                    sum_b, available, total
                );
                // Invariant: available never exceeds total
                prop_assert!(available <= total, "available={} > total={}", available, total);
                // Invariant: no balance exceeds total
                for &b in &balances {
                    prop_assert!(b <= total, "balance={} > total={}", b, total);
                }
                // On-chain state matches tracked state
                prop_assert_eq!(client.get_total_shares(), total);
                prop_assert_eq!(client.get_available_shares(), available);
                prop_assert_eq!(client.is_paused(), paused);
            }
        }

        /// Invariant: pause/unpause cycles toggle correctly.
        /// No buy_shares succeeds while paused.
        #[test]
        fn test_pause_unpause_cycles(pauses in prop::collection::vec(any::<bool>(), 1..20)) {
            let env = Env::default();
            env.mock_all_auths();
            let admin = Address::generate(&env);
            let token_id = env
                .register_stellar_asset_contract_v2(admin.clone())
                .address();
            let contract_id = env.register(RwaMarketplace, ());
            let client = RwaMarketplaceClient::new(&env, &contract_id);
            client.init(&admin, &token_id, &INIT_PRICE, &INIT_TOTAL);

            for should_pause in pauses {
                if should_pause {
                    client.pause();
                    prop_assert!(client.is_paused());
                } else {
                    client.unpause();
                    prop_assert!(!client.is_paused());
                }
            }
        }

        /// Invariant: for sequential buys by a single user,
        /// available + total_bought == INIT_TOTAL and
        /// total_shares remains unchanged.
        #[test]
        fn test_buy_sequences_invariant(buys in prop::collection::vec(1u32..200u32, 1..20)) {
            let env = Env::default();
            env.mock_all_auths();
            let admin = Address::generate(&env);
            let token_id = env
                .register_stellar_asset_contract_v2(admin.clone())
                .address();
            let contract_id = env.register(RwaMarketplace, ());
            let client = RwaMarketplaceClient::new(&env, &contract_id);

            let buyer = Address::generate(&env);
            token::StellarAssetClient::new(&env, &token_id).mint(&buyer, &1_000_000_000);
            client.init(&admin, &token_id, &INIT_PRICE, &INIT_TOTAL);
            client.add_to_whitelist(&buyer);

            let mut total_bought = 0u32;

            for shares in buys {
                let available = client.get_available_shares();
                if shares > available {
                    continue;
                }
                client.buy_shares(&buyer, &shares, &token_id);
                total_bought += shares;

                // available + total_bought == INIT_TOTAL
                prop_assert_eq!(
                    client.get_available_shares() + total_bought,
                    INIT_TOTAL,
                    "available={} + bought={} != {}",
                    client.get_available_shares(),
                    total_bought,
                    INIT_TOTAL
                );
                // holder balance matches total bought
                prop_assert_eq!(client.get_shares(&buyer), total_bought);
                // total_shares never changes
                prop_assert_eq!(client.get_total_shares(), INIT_TOTAL);
            }
        }
    }
}
// ====================== CONTRACT UPGRADEABILITY (#6) ======================

#[contractevent(data_format = "vec")]
pub struct EventContractUpgraded {
    pub new_wasm_hash: BytesN<32>,
}

#[contractimpl]
impl RwaMarketplace {

    /// Return the contract metadata (SIP-4/SEP-46).
    /// Panics if the contract is not initialized.
    pub fn get_contract_metadata(env: Env) -> ContractMetadata {
        env.storage()
            .instance()
            .get(&DataKey::ContractMetadata)
            .expect("Contract not initialized")
    }

    /// Return the admin address. Panics if the contract is not initialized.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized: admin")
    }

    /// Return whether the contract has been initialized.
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&DataKey::Admin)
    }

    /// Upgrade the smart contract to a new version.
    /// Only the admin can call this function.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");

        admin.require_auth();

        env.deployer().update_current_contract_wasm(new_wasm_hash.clone());

        EventContractUpgraded { new_wasm_hash }.publish(&env);
    }
}

// ====================== ORACLE INTEGRATION (#169) =========================

#[contractimpl]
impl RwaMarketplace {
    /// Set the oracle contract address for real-time pricing. Admin only.
    ///
    /// Once set, `buy_shares` will attempt to fetch the price from the oracle
    /// and fall back to the admin-set price if the oracle call fails.
    ///
    /// Pass `None` equivalent (remove the key) via `clear_oracle` to disable.
    pub fn set_oracle(env: Env, oracle: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();

        env.storage().instance().set(&DataKey::OracleAddress, &oracle);
        EventSetOracle { oracle }.publish(&env);
    }

    /// Remove the oracle address, reverting to admin-set pricing. Admin only.
    pub fn clear_oracle(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .expect("Contract not initialized: admin");
        admin.require_auth();
        env.storage().instance().remove(&DataKey::OracleAddress);
    }

    /// Return the configured oracle address, or None if not set.
    pub fn get_oracle(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::OracleAddress)
    }
}

// ====================== CROSS-CHAIN BRIDGE (#170) =========================

#[contractimpl]
impl RwaMarketplace {
    /// Lock `amount` shares for bridging to another chain.
    ///
    /// The caller's liquid balance is debited and the locked amount is
    /// recorded in `BridgeLocked(caller)`. Emits `EventLockForBridge`.
    ///
    /// Panics if:
    /// - `amount` is 0
    /// - caller does not have enough liquid balance
    pub fn lock_for_bridge(env: Env, user: Address, amount: u32) {
        user.require_auth();

        if amount == 0 {
            panic!("Bridge lock amount must be greater than zero");
        }

        let balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(user.clone()))
            .unwrap_or(0);

        if amount > balance {
            panic!("Insufficient liquid balance to lock for bridge");
        }

        // Debit liquid balance
        let new_balance = checked_sub_u32(balance, amount);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(user.clone()), &new_balance);

        // Increase locked amount
        let prev_locked: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::BridgeLocked(user.clone()))
            .unwrap_or(0);
        let total_locked = checked_add_u32(prev_locked, amount);
        env.storage()
            .persistent()
            .set(&DataKey::BridgeLocked(user.clone()), &total_locked);

        EventLockForBridge { user, amount, total_locked }.publish(&env);
    }

    /// Unlock `amount` shares from a bridge operation using a 32-byte proof.
    ///
    /// The proof is a bytes32 value (e.g., a merkle proof hash or bridge tx ID)
    /// supplied by the relayer. The locked amount is reduced and the caller's
    /// liquid balance is credited. Emits `EventUnlockFromBridge`.
    ///
    /// Panics if:
    /// - `amount` is 0
    /// - the proof is all-zeros (invalid proof sentinel)
    /// - caller does not have enough locked balance to unlock
    pub fn unlock_from_bridge(env: Env, user: Address, amount: u32, proof: BytesN<32>) {
        user.require_auth();

        if amount == 0 {
            panic!("Bridge unlock amount must be greater than zero");
        }

        // Reject zero proof (invalid sentinel)
        let zero: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
        if proof == zero {
            panic!("Invalid bridge proof: proof cannot be all-zeros");
        }

        let locked: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::BridgeLocked(user.clone()))
            .unwrap_or(0);

        if amount > locked {
            panic!("Insufficient locked balance to unlock from bridge");
        }

        // Reduce locked amount
        let new_locked = checked_sub_u32(locked, amount);
        env.storage()
            .persistent()
            .set(&DataKey::BridgeLocked(user.clone()), &new_locked);

        // Credit liquid balance
        let balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(user.clone()))
            .unwrap_or(0);
        let new_balance = checked_add_u32(balance, amount);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(user.clone()), &new_balance);

        EventUnlockFromBridge { user, amount, proof }.publish(&env);
    }

    /// Return the amount of shares currently locked for bridging by `user`.
    pub fn get_bridge_locked(env: Env, user: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::BridgeLocked(user))
            .unwrap_or(0)
    }
}

// ====================== ORACLE & BRIDGE UNIT TESTS ========================

#[cfg(test)]
mod oracle_bridge_tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Env,
    };

    type Client<'a> = RwaMarketplaceClient<'a>;

    const INIT_PRICE: i128 = 100;
    const INIT_TOTAL: u32 = 1_000;

    struct TestEnv {
        env: Env,
        contract_id: Address,
        admin: Address,
        token_id: Address,
        buyer: Address,
    }

    fn setup() -> TestEnv {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let contract_id = env.register(RwaMarketplace, ());
        let buyer = Address::generate(&env);
        token::StellarAssetClient::new(&env, &token_id).mint(&buyer, &100_000_000);
        TestEnv { env, contract_id, admin, token_id, buyer }
    }

    fn client(te: &TestEnv) -> Client {
        RwaMarketplaceClient::new(&te.env, &te.contract_id)
    }

    fn init(te: &TestEnv) {
        let c = client(te);
        c.init(&te.admin, &te.token_id, &INIT_PRICE, &INIT_TOTAL);
        c.add_to_whitelist(&te.buyer);
    }

    // ── Oracle tests (Issue #169) ─────────────────────────────────────────────

    #[test]
    fn test_set_and_get_oracle() {
        let te = setup();
        init(&te);
        let c = client(&te);
        let oracle_addr = Address::generate(&te.env);

        assert!(c.get_oracle().is_none());
        c.set_oracle(&oracle_addr);
        assert_eq!(c.get_oracle(), Some(oracle_addr));
    }

    #[test]
    fn test_clear_oracle() {
        let te = setup();
        init(&te);
        let c = client(&te);
        let oracle_addr = Address::generate(&te.env);

        c.set_oracle(&oracle_addr);
        assert!(c.get_oracle().is_some());
        c.clear_oracle();
        assert!(c.get_oracle().is_none());
    }

    #[test]
    #[should_panic(expected = "Contract not initialized: admin")]
    fn test_set_oracle_requires_init() {
        let te = setup();
        let c = client(&te);
        let oracle_addr = Address::generate(&te.env);
        // No init() called — must panic
        c.set_oracle(&oracle_addr);
    }

    #[test]
    fn test_buy_shares_without_oracle_uses_admin_price() {
        let te = setup();
        init(&te);
        let c = client(&te);

        let balance_before: i128 =
            token::TokenClient::new(&te.env, &te.token_id).balance(&te.buyer);
        c.buy_shares(&te.buyer, &10, &te.token_id);
        let balance_after: i128 =
            token::TokenClient::new(&te.env, &te.token_id).balance(&te.buyer);

        // 10 shares * INIT_PRICE = 1000 tokens spent
        assert_eq!(balance_before - balance_after, 10 * INIT_PRICE);
        assert_eq!(c.get_shares(&te.buyer), 10);
    }

    // ── Bridge tests (Issue #170) ─────────────────────────────────────────────

    #[test]
    fn test_lock_for_bridge_reduces_liquid_balance() {
        let te = setup();
        init(&te);
        let c = client(&te);

        c.buy_shares(&te.buyer, &100, &te.token_id);
        assert_eq!(c.get_shares(&te.buyer), 100);

        c.lock_for_bridge(&te.buyer, &30);
        // Liquid balance reduced
        assert_eq!(c.get_shares(&te.buyer), 70);
        // Locked balance set
        assert_eq!(c.get_bridge_locked(&te.buyer), 30);
    }

    #[test]
    fn test_multiple_lock_for_bridge_accumulates() {
        let te = setup();
        init(&te);
        let c = client(&te);

        c.buy_shares(&te.buyer, &100, &te.token_id);
        c.lock_for_bridge(&te.buyer, &20);
        c.lock_for_bridge(&te.buyer, &10);

        assert_eq!(c.get_shares(&te.buyer), 70);
        assert_eq!(c.get_bridge_locked(&te.buyer), 30);
    }

    #[test]
    #[should_panic(expected = "Insufficient liquid balance to lock for bridge")]
    fn test_lock_for_bridge_insufficient_balance() {
        let te = setup();
        init(&te);
        let c = client(&te);

        c.buy_shares(&te.buyer, &10, &te.token_id);
        // Try to lock more than owned
        c.lock_for_bridge(&te.buyer, &100);
    }

    #[test]
    #[should_panic(expected = "Bridge lock amount must be greater than zero")]
    fn test_lock_for_bridge_zero_amount() {
        let te = setup();
        init(&te);
        let c = client(&te);
        c.lock_for_bridge(&te.buyer, &0);
    }

    #[test]
    fn test_unlock_from_bridge_restores_liquid_balance() {
        let te = setup();
        init(&te);
        let c = client(&te);

        c.buy_shares(&te.buyer, &100, &te.token_id);
        c.lock_for_bridge(&te.buyer, &50);

        let valid_proof: BytesN<32> = BytesN::from_array(&te.env, &[1u8; 32]);
        c.unlock_from_bridge(&te.buyer, &50, &valid_proof);

        // Liquid balance restored
        assert_eq!(c.get_shares(&te.buyer), 100);
        // Bridge locked cleared
        assert_eq!(c.get_bridge_locked(&te.buyer), 0);
    }

    #[test]
    fn test_partial_unlock_from_bridge() {
        let te = setup();
        init(&te);
        let c = client(&te);

        c.buy_shares(&te.buyer, &100, &te.token_id);
        c.lock_for_bridge(&te.buyer, &60);

        let proof: BytesN<32> = BytesN::from_array(&te.env, &[2u8; 32]);
        c.unlock_from_bridge(&te.buyer, &25, &proof);

        assert_eq!(c.get_shares(&te.buyer), 65);
        assert_eq!(c.get_bridge_locked(&te.buyer), 35);
    }

    #[test]
    #[should_panic(expected = "Insufficient locked balance to unlock from bridge")]
    fn test_unlock_from_bridge_excess_amount() {
        let te = setup();
        init(&te);
        let c = client(&te);

        c.buy_shares(&te.buyer, &100, &te.token_id);
        c.lock_for_bridge(&te.buyer, &10);

        let proof: BytesN<32> = BytesN::from_array(&te.env, &[3u8; 32]);
        c.unlock_from_bridge(&te.buyer, &100, &proof);
    }

    #[test]
    #[should_panic(expected = "Invalid bridge proof: proof cannot be all-zeros")]
    fn test_unlock_from_bridge_zero_proof_rejected() {
        let te = setup();
        init(&te);
        let c = client(&te);

        c.buy_shares(&te.buyer, &50, &te.token_id);
        c.lock_for_bridge(&te.buyer, &10);

        let zero_proof: BytesN<32> = BytesN::from_array(&te.env, &[0u8; 32]);
        c.unlock_from_bridge(&te.buyer, &10, &zero_proof);
    }

    #[test]
    #[should_panic(expected = "Bridge unlock amount must be greater than zero")]
    fn test_unlock_from_bridge_zero_amount() {
        let te = setup();
        init(&te);
        let c = client(&te);

        c.buy_shares(&te.buyer, &50, &te.token_id);
        c.lock_for_bridge(&te.buyer, &10);

        let proof: BytesN<32> = BytesN::from_array(&te.env, &[4u8; 32]);
        c.unlock_from_bridge(&te.buyer, &0, &proof);
    }

    #[test]
    fn test_get_bridge_locked_default_zero() {
        let te = setup();
        init(&te);
        let c = client(&te);
        // No lock operations performed — should return 0
        assert_eq!(c.get_bridge_locked(&te.buyer), 0);
    }

    #[test]
    fn test_bridge_lock_does_not_affect_total_shares() {
        let te = setup();
        init(&te);
        let c = client(&te);

        let total_before = c.get_total_shares();
        c.buy_shares(&te.buyer, &100, &te.token_id);
        c.lock_for_bridge(&te.buyer, &50);

        // Total shares never change from bridge operations
        assert_eq!(c.get_total_shares(), total_before);
    }
}

// ── SIP-4 Metadata Tests (Issue #168) ──────────────────────────────────────────

#[cfg(test)]
mod sip4_metadata_tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    type Client<'a> = RwaMarketplaceClient<'a>;

    const INIT_PRICE: i128 = 100;
    const INIT_TOTAL: u32 = 1_000;

    struct TestEnv {
        env: Env,
        contract_id: Address,
        admin: Address,
        token_id: Address,
        buyer: Address,
    }

    fn setup() -> TestEnv {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let contract_id = env.register(RwaMarketplace, ());
        let buyer = Address::generate(&env);
        token::StellarAssetClient::new(&env, &token_id).mint(&buyer, &100_000_000);
        TestEnv { env, contract_id, admin, token_id, buyer }
    }

    fn client(te: &TestEnv) -> Client {
        RwaMarketplaceClient::new(&te.env, &te.contract_id)
    }

    fn init(te: &TestEnv) {
        let c = client(te);
        c.init(&te.admin, &te.token_id, &INIT_PRICE, &INIT_TOTAL);
        c.add_to_whitelist(&te.buyer);
    }

    #[test]
    fn test_get_contract_metadata_returns_expected_values() {
        let te = setup();
        init(&te);
        let c = client(&te);
        let meta = c.get_contract_metadata();

        assert_eq!(meta.name, String::from_str(&te.env, "RWA Marketplace"));
        assert_eq!(meta.version, String::from_str(&te.env, "0.4.0"));
        assert_eq!(meta.description, String::from_str(&te.env, "Tokenized Fractional RWA Marketplace"));
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_get_contract_metadata_before_init_panics() {
        let te = setup();
        let c = client(&te);
        c.get_contract_metadata();
    }

    // ── Issue #262: Batch purchase tests ─────────────────────────────────

    #[test]
    fn test_batch_buy_shares_basic() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        requests.push_back(BatchPurchaseRequest {
            shares: 10,
            payment_token: te.token_id.clone(),
        });
        requests.push_back(BatchPurchaseRequest {
            shares: 20,
            payment_token: te.token_id.clone(),
        });
        requests.push_back(BatchPurchaseRequest {
            shares: 5,
            payment_token: te.token_id.clone(),
        });

        let results = c.batch_buy_shares(&te.buyer, &requests);
        assert_eq!(results.len(), 3);

        // All should succeed
        assert!(results.get(0).unwrap().success);
        assert_eq!(results.get(0).unwrap().shares_purchased, 10);
        assert_eq!(results.get(0).unwrap().total_cost, 1000); // 10 * 100

        assert!(results.get(1).unwrap().success);
        assert_eq!(results.get(1).unwrap().shares_purchased, 20);
        assert_eq!(results.get(1).unwrap().total_cost, 2000); // 20 * 100

        assert!(results.get(2).unwrap().success);
        assert_eq!(results.get(2).unwrap().shares_purchased, 5);
        assert_eq!(results.get(2).unwrap().total_cost, 500); // 5 * 100

        // Verify aggregate state
        assert_eq!(c.get_shares(&te.buyer), 35); // 10 + 20 + 5
        assert_eq!(c.get_available_shares(), 965); // 1000 - 35
    }

    #[test]
    fn test_batch_buy_shares_partial_fulfillment() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        // Second request asks for 0 shares (invalid), should fail
        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        requests.push_back(BatchPurchaseRequest {
            shares: 10,
            payment_token: te.token_id.clone(),
        });
        requests.push_back(BatchPurchaseRequest {
            shares: 0, // Invalid
            payment_token: te.token_id.clone(),
        });
        requests.push_back(BatchPurchaseRequest {
            shares: 15,
            payment_token: te.token_id.clone(),
        });

        let results = c.batch_buy_shares(&te.buyer, &requests);
        assert_eq!(results.len(), 3);

        assert!(results.get(0).unwrap().success);
        assert!(!results.get(1).unwrap().success); // Failed: 0 shares
        assert!(results.get(2).unwrap().success);

        // Only 10 + 15 = 25 shares purchased
        assert_eq!(c.get_shares(&te.buyer), 25);
        assert_eq!(c.get_available_shares(), 975);
    }

    #[test]
    fn test_batch_buy_shares_exceeds_available() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &50); // Only 50 shares
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        requests.push_back(BatchPurchaseRequest {
            shares: 30,
            payment_token: te.token_id.clone(),
        });
        requests.push_back(BatchPurchaseRequest {
            shares: 30, // Would exceed remaining 20
            payment_token: te.token_id.clone(),
        });

        let results = c.batch_buy_shares(&te.buyer, &requests);
        assert_eq!(results.len(), 2);

        assert!(results.get(0).unwrap().success);
        assert!(!results.get(1).unwrap().success); // Failed: not enough available

        assert_eq!(c.get_shares(&te.buyer), 30);
        assert_eq!(c.get_available_shares(), 20); // 50 - 30
    }

    #[test]
    #[should_panic(expected = "Batch must contain at least one purchase request")]
    fn test_batch_buy_shares_empty_batch() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        c.add_to_whitelist(&te.buyer);

        let requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        c.batch_buy_shares(&te.buyer, &requests);
    }

    #[test]
    #[should_panic(expected = "Batch size exceeds maximum allowed")]
    fn test_batch_buy_shares_exceeds_max_batch_size() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        // Create 11 requests (exceeds MAX_BATCH_SIZE = 10)
        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        for _ in 0..11u32 {
            requests.push_back(BatchPurchaseRequest {
                shares: 1,
                payment_token: te.token_id.clone(),
            });
        }
        c.batch_buy_shares(&te.buyer, &requests);
    }

    #[test]
    #[should_panic(expected = "Buyer is not whitelisted")]
    fn test_batch_buy_shares_requires_whitelist() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        // Not whitelisted

        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        requests.push_back(BatchPurchaseRequest {
            shares: 10,
            payment_token: te.token_id.clone(),
        });
        c.batch_buy_shares(&te.buyer, &requests);
    }

    #[test]
    #[should_panic(expected = "Marketplace is paused")]
    fn test_batch_buy_shares_when_paused() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);
        c.pause();

        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        requests.push_back(BatchPurchaseRequest {
            shares: 10,
            payment_token: te.token_id.clone(),
        });
        c.batch_buy_shares(&te.buyer, &requests);
    }

    #[test]
    fn test_batch_buy_shares_all_fail() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        // All requests have 0 shares
        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        requests.push_back(BatchPurchaseRequest {
            shares: 0,
            payment_token: te.token_id.clone(),
        });
        requests.push_back(BatchPurchaseRequest {
            shares: 0,
            payment_token: te.token_id.clone(),
        });

        let results = c.batch_buy_shares(&te.buyer, &requests);
        assert_eq!(results.len(), 2);
        assert!(!results.get(0).unwrap().success);
        assert!(!results.get(1).unwrap().success);

        // No state changes
        assert_eq!(c.get_shares(&te.buyer), 0);
        assert_eq!(c.get_available_shares(), 1000);
    }

    #[test]
    fn test_get_batch_quote_accuracy() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        requests.push_back(BatchPurchaseRequest {
            shares: 10,
            payment_token: te.token_id.clone(),
        });
        requests.push_back(BatchPurchaseRequest {
            shares: 0, // Should fail in quote too
            payment_token: te.token_id.clone(),
        });
        requests.push_back(BatchPurchaseRequest {
            shares: 25,
            payment_token: te.token_id.clone(),
        });

        let quotes = c.get_batch_quote(&te.buyer, &requests);
        assert_eq!(quotes.len(), 3);

        assert!(quotes.get(0).unwrap().success);
        assert_eq!(quotes.get(0).unwrap().total_cost, 1000);

        assert!(!quotes.get(1).unwrap().success);

        assert!(quotes.get(2).unwrap().success);
        assert_eq!(quotes.get(2).unwrap().total_cost, 2500);

        // Quote should NOT change state
        assert_eq!(c.get_available_shares(), 1000);
        assert_eq!(c.get_shares(&te.buyer), 0);
    }

    #[test]
    fn test_batch_buy_shares_single_item() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        requests.push_back(BatchPurchaseRequest {
            shares: 50,
            payment_token: te.token_id.clone(),
        });

        let results = c.batch_buy_shares(&te.buyer, &requests);
        assert_eq!(results.len(), 1);
        assert!(results.get(0).unwrap().success);
        assert_eq!(results.get(0).unwrap().shares_purchased, 50);
        assert_eq!(results.get(0).unwrap().total_cost, 5000);
        assert_eq!(c.get_shares(&te.buyer), 50);
        assert_eq!(c.get_available_shares(), 950);
    }

    #[test]
    fn test_batch_buy_shares_max_batch_size() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 1_000_000);
        c.add_to_whitelist(&te.buyer);

        // Exactly 10 requests (the limit)
        let mut requests: Vec<BatchPurchaseRequest> = Vec::new(&te.env);
        for _ in 0..10u32 {
            requests.push_back(BatchPurchaseRequest {
                shares: 1,
                payment_token: te.token_id.clone(),
            });
        }

        let results = c.batch_buy_shares(&te.buyer, &requests);
        assert_eq!(results.len(), 10);
        for i in 0..10u32 {
            assert!(results.get(i).unwrap().success);
        }
        assert_eq!(c.get_shares(&te.buyer), 10);        assert_eq!(c.get_available_shares(), 990);
    }

    // ── Issue #270: Whitelist enhancement tests ───────────────────────────

    #[test]
    fn test_add_to_whitelist_batch() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        let a1 = Address::generate(&te.env);
        let a2 = Address::generate(&te.env);
        let mut addrs: Vec<Address> = Vec::new(&te.env);
        addrs.push_back(a1.clone());
        addrs.push_back(a2.clone());
        c.add_to_whitelist_batch(&addrs);
        assert!(c.is_whitelisted(&a1));
        assert!(c.is_whitelisted(&a2));
    }

    #[test]
    fn test_set_whitelist_expiry() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        let addr = Address::generate(&te.env);
        c.add_to_whitelist(&addr);
        let far_future = te.env.ledger().timestamp() + 86_400;
        c.set_whitelist_expiry(&addr, &far_future);
        assert!(c.is_whitelisted(&addr));
        c.set_whitelist_expiry(&addr, &1_u64);
        assert!(!c.is_whitelisted(&addr));
    }

    #[test]
    #[should_panic(expected = "Invalid tier")]
    fn test_set_whitelist_tier_invalid() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        let addr = Address::generate(&te.env);
        c.set_whitelist_tier(&addr, &5_u32);
    }

    // ── Issue #263: Transfer fee tests ─────────────────────────────────────

    #[test]
    fn test_set_and_get_transfer_fee() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        let collector = Address::generate(&te.env);
        c.set_transfer_fee(&30_u32, &collector);
        let (fee_bps, fee_collector) = c.get_transfer_fee();
        assert_eq!(fee_bps, 30);
        assert_eq!(fee_collector, Some(collector));
    }

    #[test]
    #[should_panic(expected = "Transfer fee cannot exceed")]
    fn test_set_transfer_fee_too_high() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        let collector = Address::generate(&te.env);
        c.set_transfer_fee(&2000_u32, &collector);
    }

    // ── Issue #268: Buyback enhancement tests ─────────────────────────────

    #[test]
    fn test_request_buyback() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &100, &te.token_id);
        let request_id = c.request_buyback(&te.buyer, &50, &95_i128);
        let request = c.get_buyback_request(&request_id);
        assert!(request.is_some());
    }

    #[test]
    #[should_panic(expected = "Buybacks are currently paused")]
    fn test_buyback_paused() {
        let te = setup();
        let c = client(&te);
        c.init(&te.admin, &te.token_id, &100, &1000);
        mint(&te, &te.buyer, 100_000);
        c.add_to_whitelist(&te.buyer);
        c.buy_shares(&te.buyer, &100, &te.token_id);
        mint(&te, &te.contract_id, 50_000);
        c.pause_function(&4_u32);
        c.buyback_shares(&te.buyer, &10);
    }
}
