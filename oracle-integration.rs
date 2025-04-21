// Oracle integration for price feeds
use anchor_lang::prelude::*;
use pyth_sdk_solana::{load_price_feed_from_account_info, Price, PriceFeed};
use switchboard_v2::{AggregatorAccountData, SwitchboardDecimal};

declare_id!("Orch2iNTvfE4L5YmEXp9QeJ83NRmJYGrt5x0P3jBK"); // Replace with your program ID

#[program]
pub mod openfund_oracle {
    use super::*;

    pub fn initialize_oracle_config(
        ctx: Context<InitializeOracleConfig>,
        price_feed_type: OracleFeedType,
        heartbeat_threshold_seconds: i64,
        confidence_threshold_percent: u64,
    ) -> Result<()> {
        let oracle_config = &mut ctx.accounts.oracle_config;
        oracle_config.authority = ctx.accounts.authority.key();
        oracle_config.price_feed = ctx.accounts.price_feed.key();
        oracle_config.price_feed_type = price_feed_type;
        oracle_config.heartbeat_threshold_seconds = heartbeat_threshold_seconds;
        oracle_config.confidence_threshold_percent = confidence_threshold_percent;
        oracle_config.bump = *ctx.bumps.get("oracle_config").unwrap();
        
        Ok(())
    }

    pub fn update_price_feed(
        ctx: Context<UpdatePriceFeed>,
        price_feed_type: OracleFeedType,
    ) -> Result<()> {
        let oracle_config = &mut ctx.accounts.oracle_config;
        oracle_config.price_feed = ctx.accounts.new_price_feed.key();
        oracle_config.price_feed_type = price_feed_type;
        
        Ok(())
    }

    pub fn update_thresholds(
        ctx: Context<UpdateThresholds>,
        heartbeat_threshold_seconds: i64,
        confidence_threshold_percent: u64,
    ) -> Result<()> {
        let oracle_config = &mut ctx.accounts.oracle_config;
        oracle_config.heartbeat_threshold_seconds = heartbeat_threshold_seconds;
        oracle_config.confidence_threshold_percent = confidence_threshold_percent;
        
        Ok(())
    }

    pub fn get_price(ctx: Context<GetPrice>) -> Result<PriceData> {
        let oracle_config = &ctx.accounts.oracle_config;
        let clock = Clock::get()?;
        
        match oracle_config.price_feed_type {
            OracleFeedType::Pyth => {
                // Get price from Pyth
                let price_feed: PriceFeed = load_price_feed_from_account_info(
                    &ctx.accounts.price_feed
                ).map_err(|_| ErrorCode::PriceUnavailable)?;
                
                let price_data = price_feed.get_price_unchecked();
                
                // Verify freshness
                let last_update_time = price_data.publish_time;
                let time_since_update = clock.unix_timestamp - last_update_time;
                require!(
                    time_since_update <= oracle_config.heartbeat_threshold_seconds,
                    ErrorCode::StalePrice
                );
                
                // Verify confidence
                let confidence_ratio = (price_data.conf as f64 / price_data.price as f64) * 100.0;
                require!(
                    confidence_ratio <= oracle_config.confidence_threshold_percent as f64,
                    ErrorCode::LowConfidence
                );
                
                // Return price data
                Ok(PriceData {
                    price: price_data.price,
                    confidence: price_data.conf,
                    exponent: price_data.expo,
                    last_updated: price_data.publish_time,
                })
            },
            OracleFeedType::Switchboard => {
                // Get price from Switchboard
                let aggregator = AggregatorAccountData::new(ctx.accounts.price_feed.clone())
                    .map_err(|_| ErrorCode::PriceUnavailable)?;
                
                let latest_result = aggregator.get_result()
                    .map_err(|_| ErrorCode::PriceUnavailable)?;
                
                // Verify freshness
                let last_update_time = aggregator.latest_confirmed_round.unwrap().round_open_timestamp;
                let time_since_update = clock.unix_timestamp - (last_update_time as i64);
                require!(
                    time_since_update <= oracle_config.heartbeat_threshold_seconds,
                    ErrorCode::StalePrice
                );
                
                // Get confidence interval
                let latest_confidence_interval = aggregator.latest_confidence_interval().unwrap();
                let confidence_ratio = (latest_confidence_interval.to_f64() / latest_result.to_f64()) * 100.0;
                require!(
                    confidence_ratio <= oracle_config.confidence_threshold_percent as f64,
                    ErrorCode::LowConfidence
                );
                
                // Return price data
                let decimal = aggregator.latest_value().unwrap();
                Ok(PriceData {
                    price: decimal.mantissa,
                    confidence: latest_confidence_interval.mantissa,
                    exponent: decimal.scale as i32,
                    last_updated: last_update_time as i64,
                })
            },
            OracleFeedType::Chainlink => {
                // Chainlink implementation would go here
                // For simplicity, we're using a placeholder
                return Err(ErrorCode::UnsupportedOracle.into());
            }
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum OracleFeedType {
    Pyth,
    Switchboard,
    Chainlink,  // For future expansion
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PriceData {
    pub price: i64,
    pub confidence: u64,
    pub exponent: i32,
    pub last_updated: i64,
}

#[account]
pub struct OracleConfig {
    pub authority: Pubkey,                     // Admin who can update oracle settings
    pub price_feed: Pubkey,                    // Oracle price feed account
    pub price_feed_type: OracleFeedType,       // Type of oracle used
    pub heartbeat_threshold_seconds: i64,      // Maximum age of price data
    pub confidence_threshold_percent: u64,     // Maximum confidence interval as percentage
    pub bump: u8,                              // PDA bump seed
}

#[derive(Accounts)]
pub struct InitializeOracleConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<OracleConfig>() + 8, // Add some padding
        seeds = [
            b"oracle_config".as_ref(),
            token_pair.key().as_ref(),
        ],
        bump
    )]
    pub oracle_config: Account<'info, OracleConfig>,
    
    /// CHECK: This represents a token pair (e.g., SOL/USDC)
    pub token_pair: AccountInfo<'info>,
    
    /// CHECK: This is an oracle price feed account
    pub price_feed: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdatePriceFeed<'info> {
    #[account(
        mut,
        seeds = [
            b"oracle_config".as_ref(),
            token_pair.key().as_ref(),
        ],
        bump = oracle_config.bump,
        constraint = oracle_config.authority == authority.key()
    )]
    pub oracle_config: Account<'info, OracleConfig>,
    
    /// CHECK: This represents a token pair (e.g., SOL/USDC)
    pub token_pair: AccountInfo<'info>,
    
    /// CHECK: This is the new oracle price feed account
    pub new_price_feed: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateThresholds<'info> {
    #[account(
        mut,
        seeds = [
            b"oracle_config".as_ref(),
            token_pair.key().as_ref(),
        ],
        bump = oracle_config.bump,
        constraint = oracle_config.authority == authority.key()
    )]
    pub oracle_config: Account<'info, OracleConfig>,
    
    /// CHECK: This represents a token pair (e.g., SOL/USDC)
    pub token_pair: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetPrice<'info> {
    #[account(
        seeds = [
            b"oracle_config".as_ref(),
            token_pair.key().as_ref(),
        ],
        bump = oracle_config.bump
    )]
    pub oracle_config: Account<'info, OracleConfig>,
    
    /// CHECK: This represents a token pair (e.g., SOL/USDC)
    pub token_pair: AccountInfo<'info>,
    
    /// CHECK: This is the oracle price feed account
    #[account(
        constraint = price_feed.key() == oracle_config.price_feed
    )]
    pub price_feed: AccountInfo<'info>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Price data is unavailable")]
    PriceUnavailable,
    #[msg("Price data is stale")]
    StalePrice,
    #[msg("Price confidence is too low")]
    LowConfidence,
    #[msg("Oracle type is unsupported")]
    UnsupportedOracle,
}
