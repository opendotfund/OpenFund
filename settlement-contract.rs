// Settlement contract for the OpenFund DEX on Solana
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use std::collections::BTreeMap;

declare_id!("Sett1emEnt5KGj8KLEXfCgPZR4uNJE94a4yHgtB"); // Replace with your program ID

#[program]
pub mod openfund_settlement {
    use super::*;

    pub fn initialize_settlement_manager(
        ctx: Context<InitializeSettlementManager>,
        settlement_fee_bps: u16,
        settlement_window_seconds: i64,
    ) -> Result<()> {
        let settlement_manager = &mut ctx.accounts.settlement_manager;
        settlement_manager.authority = ctx.accounts.authority.key();
        settlement_manager.settlement_fee_bps = settlement_fee_bps;
        settlement_manager.settlement_window_seconds = settlement_window_seconds;
        settlement_manager.fee_treasury = ctx.accounts.fee_treasury.key();
        settlement_manager.active = true;
        settlement_manager.bump = *ctx.bumps.get("settlement_manager").unwrap();
        
        Ok(())
    }

    pub fn update_settlement_params(
        ctx: Context<UpdateSettlementParams>,
        settlement_fee_bps: u16,
        settlement_window_seconds: i64,
    ) -> Result<()> {
        require!(
            settlement_fee_bps <= 100, // Max 1% settlement fee
            ErrorCode::FeeTooHigh
        );
        
        let settlement_manager = &mut ctx.accounts.settlement_manager;
        settlement_manager.settlement_fee_bps = settlement_fee_bps;
        settlement_manager.settlement_window_seconds = settlement_window_seconds;
        
        Ok(())
    }

    pub fn update_fee_treasury(
        ctx: Context<UpdateFeeTreasury>
    ) -> Result<()> {
        let settlement_manager = &mut ctx.accounts.settlement_manager;
        settlement_manager.fee_treasury = ctx.accounts.new_fee_treasury.key();
        
        Ok(())
    }

    pub fn toggle_settlement_status(
        ctx: Context<ToggleSettlementStatus>,
        active: bool,
    ) -> Result<()> {
        let settlement_manager = &mut ctx.accounts.settlement_manager;
        settlement_manager.active = active;
        
        Ok(())
    }

    pub fn create_order(
        ctx: Context<CreateOrder>,
        amount_in: u64,
        min_amount_out: u64,
        direction: OrderDirection,
        expiry_timestamp: i64,
    ) -> Result<()> {
        let settlement_manager = &ctx.accounts.settlement_manager;
        require!(settlement_manager.active, ErrorCode::SettlementPaused);
        
        let clock = Clock::get()?;
        require!(
            expiry_timestamp > clock.unix_timestamp,
            ErrorCode::InvalidExpiry
        );
        
        let max_expiry = clock.unix_timestamp + settlement_manager.settlement_window_seconds;
        require!(
            expiry_timestamp <= max_expiry,
            ErrorCode::ExpiryTooLong
        );
        
        let order = &mut ctx.accounts.order;
        order.user = ctx.accounts.user.key();
        order.pool = ctx.accounts.pool.key();
        order.token_a_mint = ctx.accounts.token_a_mint.key();
        order.token_b_mint = ctx.accounts.token_b_mint.key();
        order.amount_in = amount_in;
        order.min_amount_out = min_amount_out;
        order.direction = direction;
        order.status = OrderStatus::Open;
        order.created_at = clock.unix_timestamp;
        order.expiry_timestamp = expiry_timestamp;
        order.bump = *ctx.bumps.get("order").unwrap();
        
        // Transfer token_in to the order escrow account
        let (source_token_account, escrow_token_account) = match direction {
            OrderDirection::AtoB => (
                ctx.accounts.user_token_a.to_account_info(),
                ctx.accounts.escrow_token_a.to_account_info(),
            ),
            OrderDirection::BtoA => (
                ctx.accounts.user_token_b.to_account_info(),
                ctx.accounts.escrow_token_b.to_account_info(),
            ),
        };
        
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: source_token_account,
                    to: escrow_token_account,
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_in,
        )?;
        
        Ok(())
    }

    pub fn cancel_order(ctx: Context<CancelOrder>) -> Result<()> {
        let order = &mut ctx.accounts.order;
        require!(
            order.status == OrderStatus::Open,
            ErrorCode::InvalidOrderStatus
        );
        
        // Update order status
        order.status = OrderStatus::Cancelled;
        
        // Return tokens from escrow to user
        let (escrow_token_account, user_token_account) = match order.direction {
            OrderDirection::AtoB => (
                ctx.accounts.escrow_token_a.to_account_info(),
                ctx.accounts.user_token_a.to_account_info(),
            ),
            OrderDirection::BtoA => (
                ctx.accounts.escrow_token_b.to_account_info(),
                ctx.accounts.user_token_b.to_account_info(),
            ),
        };
        
        // Transfer tokens from escrow back to user
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: escrow_token_account,
                    to: user_token_account,
                    authority: ctx.accounts.order_authority.to_account_info(),
                },
                &[&[
                    b"order_authority",
                    order.key().as_ref(),
                    &[ctx.bumps["order_authority"]],
                ]],
            ),
            order.amount_in,
        )?;
        
        Ok(())
    }

    pub fn execute_order(
        ctx: Context<ExecuteOrder>,
        amount_out: u64,
    ) -> Result<()> {
        let settlement_manager = &ctx.accounts.settlement_manager;
        require!(settlement_manager.active, ErrorCode::SettlementPaused);
        
        let order = &mut ctx.accounts.order;
        require!(
            order.status == OrderStatus::Open,
            ErrorCode::InvalidOrderStatus
        );
        
        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp <= order.expiry_timestamp,
            ErrorCode::OrderExpired
        );
        
        require!(
            amount_out >= order.min_amount_out,
            ErrorCode::SlippageExceeded
        );
        
        // Calculate settlement fee
        let fee_amount = (amount_out as u128)
            .checked_mul(settlement_manager.settlement_fee_bps as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap() as u64;
            
        let amount_out_after_fee = amount_out.checked_sub(fee_amount).unwrap();
        
        // Update order status
        order.status = OrderStatus::Executed;
        order.execution_amount = amount_out;
        order.execution_fee = fee_amount;
        order.executed_at = clock.unix_timestamp;
        
        // Determine which tokens are going where
        let (escrow_source, pool_target, pool_source, user_target, treasury_target) = match order.direction {
            OrderDirection::AtoB => (
                ctx.accounts.escrow_token_a.to_account_info(),
                ctx.accounts.pool_token_a.to_account_info(),
                ctx.accounts.pool_token_b.to_account_info(),
                ctx.accounts.user_token_b.to_account_info(),
                ctx.accounts.treasury_token_b.to_account_info(),
            ),
            OrderDirection::BtoA => (
                ctx.accounts.escrow_token_b.to_account_info(),
                ctx.accounts.pool_token_b.to_account_info(),
                ctx.accounts.pool_token_a.to_account_info(),
                ctx.accounts.user_token_a.to_account_info(),
                ctx.accounts.treasury_token_a.to_account_info(),
            ),
        };
        
        // Transfer tokens from escrow to pool
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: escrow_source,
                    to: pool_target,
                    authority: ctx.accounts.order_authority.to_account_info(),
                },
                &[&[
                    b"order_authority",
                    order.key().as_ref(),
                    &[ctx.bumps["order_authority"]],
                ]],
            ),
            order.amount_in,
        )?;
        
        // Transfer tokens from pool to user
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: pool_source,
                    to: user_target,
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                &[&[
                    b"pool_authority",
                    ctx.accounts.pool.key().as_ref(),
                    &[ctx.bumps["pool_authority"]],
                ]],
            ),
            amount_out_after_fee,
        )?;
        
        // Transfer fee to treasury if there is a fee
        if fee_amount > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: pool_source,
                        to: treasury_target,
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    &[&[
                        b"pool_authority",
                        ctx.accounts.pool.key().as_ref(),
                        &[ctx.bumps["pool_authority"]],
                    ]],
                ),
                fee_amount,
            )?;
        }
        
        Ok(())
    }

    pub fn batch_execute_orders(
        ctx: Context<BatchExecuteOrders>,
        order_keys: Vec<Pubkey>,
        amounts_out: Vec<u64>,
    ) -> Result<()> {
        require!(
            order_keys.len() == amounts_out.len(),
            ErrorCode::BatchMismatch
        );
        
        // This would be implemented to process multiple orders in one transaction
        // For demonstration purposes, just showing the structure
        // In a real implementation, would need to iterate through orders and verify them
        
        Ok(())
    }

    pub fn claim_expired_orders(
        ctx: Context<ClaimExpiredOrders>,
        order_keys: Vec<Pubkey>,
    ) -> Result<()> {
        let clock = Clock::get()?;
        
        // This would be implemented to process expired orders
        // For demonstration purposes, just showing the structure
        // In a real implementation, would need to verify order expiry and return funds
        
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Copy)]
pub enum OrderDirection {
    AtoB,  // Swap from token A to token B
    BtoA,  // Swap from token B to token A
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Copy)]
pub enum OrderStatus {
    Open,
    Executed,
    Cancelled,
    Expired,
}

#[account]
pub struct SettlementManager {
    pub authority: Pubkey,                    // Admin authority
    pub settlement_fee_bps: u16,              // Fee in basis points (1/100th of a percent)
    pub settlement_window_seconds: i64,       // Maximum order lifetime
    pub fee_treasury: Pubkey,                 // Treasury account for fees
    pub active: bool,                         // Whether settlement system is active
    pub bump: u8,                             // PDA bump seed
}

#[account]
pub struct Order {
    pub user: Pubkey,                         // Order creator
    pub pool: Pubkey,                         // Pool against which the order is placed
    pub token_a_mint: Pubkey,                 // Token A mint address
    pub token_b_mint: Pubkey,                 // Token B mint address
    pub amount_in: u64,                       // Amount of input token
    pub min_amount_out: u64,                  // Minimum amount of output token (slippage protection)
    pub direction: OrderDirection,            // Whether A→B or B→A
    pub status: OrderStatus,                  // Order status
    pub created_at: i64,                      // Creation timestamp
    pub expiry_timestamp: i64,                // Expiry timestamp
    pub execution_amount: u64,                // Amount received on execution (if executed)
    pub execution_fee: u64,                   // Fee paid on execution (if executed)
    pub executed_at: i64,                     // Execution timestamp (if executed)
    pub bump: u8,                             // PDA bump seed
}

#[derive(Accounts)]
pub struct InitializeSettlementManager<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<SettlementManager>() + 8, // Add some padding
        seeds = [b"settlement_manager".as_ref()],
        bump
    )]
    pub settlement_manager: Account<'info, SettlementManager>,

    /// CHECK: This is the treasury that receives settlement fees
    pub fee_treasury: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateSettlementParams<'info> {
    #[account(
        mut,
        seeds = [b"settlement_manager".as_ref()],
        bump = settlement_manager.bump,
        constraint = settlement_manager.authority == authority.key()
    )]
    pub settlement_manager: Account<'info, SettlementManager>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateFeeTreasury<'info> {
    #[account(
        mut,
        seeds = [b"settlement_manager".as_ref()],
        bump = settlement_manager.bump,
        constraint = settlement_manager.authority == authority.key()
    )]
    pub settlement_manager: Account<'info, SettlementManager>,
    
    /// CHECK: This is the new treasury that will receive settlement fees
    pub new_fee_treasury: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct ToggleSettlementStatus<'info> {
    #[account(
        mut,
        seeds = [b"settlement_manager".as_ref()],
        bump = settlement_manager.bump,
        constraint = settlement_manager.authority == authority.key()
    )]
    pub settlement_manager: Account<'info, SettlementManager>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CreateOrder<'info> {
    #[account(
        seeds = [b"settlement_manager".as_ref()],
        bump = settlement_manager.bump
    )]
    pub settlement_manager: Account<'info, SettlementManager>,

    #[account(
        seeds = [b"pool".as_ref(), token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump,
    )]
    /// CHECK: This is the pool account that will be referenced by the order
    pub pool: AccountInfo<'info>,
    
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = user,
        space = 8 + std::mem::size_of::<Order>() + 8, // Add some padding
        seeds = [
            b"order".as_ref(),
            user.key().as_ref(),
            pool.key().as_ref(),
            &Clock::get()?.unix_timestamp.to_le_bytes()
        ],
        bump
    )]
    pub order: Account<'info, Order>,
    
    #[account(
        seeds = [
            b"order_authority".as_ref(),
            order.key().as_ref(),
        ],
        bump
    )]
    /// CHECK: This is a PDA used as authority for order escrow accounts
    pub order_authority: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = user_token_a.mint == token_a_mint.key(),
        constraint = user_token_a.owner == user.key()
    )]
    pub user_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = user_token_b.mint == token_b_mint.key(),
        constraint = user_token_b.owner == user.key()
    )]
    pub user_token_b: Account<'info, TokenAccount>,
    
    #[account(
        init,
        payer = user,
        token::mint = token_a_mint,
        token::authority = order_authority,
    )]
    pub escrow_token_a: Account<'info, TokenAccount>,
    
    #[account(
        init,
        payer = user,
        token::mint = token_b_mint,
        token::authority = order_authority,
    )]
    pub escrow_token_b: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CancelOrder<'info> {
    #[account(
        mut,
        seeds = [
            b"order".as_ref(),
            order.user.as_ref(),
            order.pool.as_ref(),
            &order.created_at.to_le_bytes()
        ],
        bump = order.bump,
        constraint = order.user == user.key()
    )]
    pub order: Account<'info, Order>,
    
    #[account(
        seeds = [
            b"order_authority".as_ref(),
            order.key().as_ref(),
        ],
        bump
    )]
    /// CHECK: This is a PDA used as authority for order escrow accounts
    pub order_authority: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = user_token_a.mint == order.token_a_mint,
        constraint = user_token_a.owner == user.key()
    )]
    pub user_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = user_token_b.mint == order.token_b_mint,
        constraint = user_token_b.owner == user.key()
    )]
    pub user_token_b: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = escrow_token_a.mint == order.token_a_mint,
        constraint = escrow_token_a.owner == order_authority.key()
    )]
    pub escrow_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = escrow_token_b.mint == order.token_b_mint,
        constraint = escrow_token_b.owner == order_authority.key()
    )]
    pub escrow_token_b: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ExecuteOrder<'info> {
    #[account(
        seeds = [b"settlement_manager".as_ref()],
        bump = settlement_manager.bump
    )]
    pub settlement_manager: Account<'info, SettlementManager>,
    
    #[account(
        mut,
        seeds = [
            b"order".as_ref(),
            order.user.as_ref(),
            order.pool.as_ref(),
            &order.created_at.to_le_bytes()
        ],
        bump = order.bump
    )]
    pub order: Account<'info, Order>,
    
    #[account(
        seeds = [
            b"order_authority".as_ref(),
            order.key().as_ref(),
        ],
        bump
    )]
    /// CHECK: This is a PDA used as authority for order escrow accounts
    pub order_authority: AccountInfo<'info>,
    
    #[account(
        seeds = [
            b"pool_authority".as_ref(),
            pool.key().as_ref(),
        ],
        bump
    )]
    /// CHECK: This is a PDA used as authority for pool accounts
    pub pool_authority: AccountInfo<'info>,
    
    #[account(
        constraint = pool.key() == order.pool
    )]
    /// CHECK: This is the pool account referenced by the order
    pub pool: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = user_token_a.mint == order.token_a_mint,
        constraint = user_token_a.owner == order.user
    )]
    pub user_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = user_token_b.mint == order.token_b_mint,
        constraint = user_token_b.owner == order.user
    )]
    pub user_token_b: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = escrow_token_a.mint == order.token_a_mint,
        constraint = escrow_token_a.owner == order_authority.key()
    )]
    pub escrow_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = escrow_token_b.mint == order.token_b_mint,
        constraint = escrow_token_b.owner == order_authority.key()
    )]
    pub escrow_token_b: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = pool_token_a.mint == order.token_a_mint
    )]
    pub pool_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = pool_token_b.mint == order.token_b_mint
    )]
    pub pool_token_b: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = treasury_token_a.mint == order.token_a_mint,
        constraint = treasury_token_a.key() == settlement_manager.fee_treasury ||
                    treasury_token_b.key() == settlement_manager.fee_treasury
    )]
    pub treasury_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = treasury_token_b.mint == order.token_b_mint,
        constraint = treasury_token_a.key() == settlement_manager.fee_treasury ||
                    treasury_token_b.key() == settlement_manager.fee_treasury
    )]
    pub treasury_token_b: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub executor: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BatchExecuteOrders<'info> {
    #[account(
        seeds = [b"settlement_manager".as_ref()],
        bump = settlement_manager.bump
    )]
    pub settlement_manager: Account<'info, SettlementManager>,
    
    // In a real implementation, would need to include all relevant accounts
    // for processing multiple orders
    
    #[account(mut)]
    pub executor: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimExpiredOrders<'info> {
    #[account(
        seeds = [b"settlement_manager".as_ref()],
        bump = settlement_manager.bump
    )]
    pub settlement_manager: Account<'info, SettlementManager>,
    
    // In a real implementation, would need to include all relevant accounts
    // for processing expired orders
    
    #[account(mut)]
    pub claimer: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Fee too high")]
    FeeTooHigh,
    #[msg("Settlement system is paused")]
    SettlementPaused,
    #[msg("Order expiry time is in the past")]
    InvalidExpiry,
    #[msg("Order expiry exceeds maximum settlement window")]
    ExpiryTooLong,
    #[msg("Order is in an invalid state for this operation")]
    InvalidOrderStatus,
    #[msg("Order has expired")]
    OrderExpired,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Batch operation arrays length mismatch")]
    BatchMismatch,
}
