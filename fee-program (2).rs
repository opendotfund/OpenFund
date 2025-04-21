// Fee collection and administration program for DEX
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("FeeCLPnVdK5QyGj8KLEXfCgPZR4uNJE94a4Xu2B"); // Replace with your program ID

#[program]
pub mod openfund_fee_management {
    use super::*;

    pub fn initialize_fee_config(
        ctx: Context<InitializeFeeConfig>,
        trading_fee_bps: u16,
        protocol_fee_pct: u16,
        lp_fee_pct: u16,
    ) -> Result<()> {
        // Validate fees
        require!(trading_fee_bps <= 1000, ErrorCode::FeeTooHigh); // Max 10%
        require!(
            protocol_fee_pct + lp_fee_pct == 100,
            ErrorCode::FeeDistributionInvalid
        ); // Must add up to 100%

        let fee_config = &mut ctx.accounts.fee_config;
        fee_config.authority = ctx.accounts.authority.key();
        fee_config.trading_fee_bps = trading_fee_bps;
        fee_config.protocol_fee_pct = protocol_fee_pct;
        fee_config.lp_fee_pct = lp_fee_pct;
        fee_config.protocol_treasury = ctx.accounts.protocol_treasury.key();
        fee_config.bump = *ctx.bumps.get("fee_config").unwrap();

        Ok(())
    }

    pub fn update_fee_config(
        ctx: Context<UpdateFeeConfig>,
        trading_fee_bps: u16,
        protocol_fee_pct: u16,
        lp_fee_pct: u16,
    ) -> Result<()> {
        // Validate fees
        require!(trading_fee_bps <= 1000, ErrorCode::FeeTooHigh); // Max 10%
        require!(
            protocol_fee_pct + lp_fee_pct == 100,
            ErrorCode::FeeDistributionInvalid
        ); // Must add up to 100%

        let fee_config = &mut ctx.accounts.fee_config;
        fee_config.trading_fee_bps = trading_fee_bps;
        fee_config.protocol_fee_pct = protocol_fee_pct;
        fee_config.lp_fee_pct = lp_fee_pct;

        Ok(())
    }

    pub fn update_treasury(
        ctx: Context<UpdateTreasury>,
    ) -> Result<()> {
        let fee_config = &mut ctx.accounts.fee_config;
        fee_config.protocol_treasury = ctx.accounts.new_protocol_treasury.key();

        Ok(())
    }

    pub fn collect_protocol_fees(
        ctx: Context<CollectProtocolFees>,
        amount: u64,
    ) -> Result<()> {
        // Transfer fees from fee vault to protocol treasury
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.fee_vault.to_account_info(),
                    to: ctx.accounts.protocol_treasury.to_account_info(),
                    authority: ctx.accounts.fee_vault_authority.to_account_info(),
                },
                &[&[
                    b"fee_vault_authority",
                    ctx.accounts.fee_config.key().as_ref(),
                    &[ctx.bumps["fee_vault_authority"]],
                ]],
            ),
            amount,
        )?;

        Ok(())
    }

    // Function to calculate fees for a swap
    // This is called by the AMM during swaps
    pub fn calculate_fees(
        ctx: Context<CalculateFees>,
        amount_in: u64,
    ) -> Result<CalculatedFees> {
        let fee_config = &ctx.accounts.fee_config;
        
        // Calculate total fee
        let total_fee = (amount_in as u128)
            .checked_mul(fee_config.trading_fee_bps as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap() as u64;
            
        // Calculate protocol portion of fee
        let protocol_fee = (total_fee as u128)
            .checked_mul(fee_config.protocol_fee_pct as u128)
            .unwrap()
            .checked_div(100)
            .unwrap() as u64;
            
        // LP portion is the remainder
        let lp_fee = total_fee.checked_sub(protocol_fee).unwrap();
        
        // Amount after fees
        let amount_after_fees = amount_in.checked_sub(total_fee).unwrap();
        
        Ok(CalculatedFees {
            total_fee,
            protocol_fee,
            lp_fee,
            amount_after_fees,
        })
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CalculatedFees {
    pub total_fee: u64,
    pub protocol_fee: u64,
    pub lp_fee: u64,
    pub amount_after_fees: u64,
}

#[account]
pub struct FeeConfig {
    pub authority: Pubkey,         // Admin who can update fee settings
    pub trading_fee_bps: u16,      // Fee in basis points (e.g., 30 = 0.3%)
    pub protocol_fee_pct: u16,     // Percentage of fee going to protocol treasury
    pub lp_fee_pct: u16,           // Percentage of fee going to liquidity providers
    pub protocol_treasury: Pubkey, // Treasury account to collect protocol fees
    pub bump: u8,                  // PDA bump seed
}

#[derive(Accounts)]
pub struct InitializeFeeConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<FeeConfig>(),
        seeds = [b"fee_config".as_ref()],
        bump
    )]
    pub fee_config: Account<'info, FeeConfig>,
    
    /// CHECK: This account will receive protocol fees
    pub protocol_treasury: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateFeeConfig<'info> {
    #[account(
        mut,
        seeds = [b"fee_config".as_ref()],
        bump = fee_config.bump,
        constraint = fee_config.authority == authority.key()
    )]
    pub fee_config: Account<'info, FeeConfig>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateTreasury<'info> {
    #[account(
        mut,
        seeds = [b"fee_config".as_ref()],
        bump = fee_config.bump,
        constraint = fee_config.authority == authority.key()
    )]
    pub fee_config: Account<'info, FeeConfig>,
    
    /// CHECK: This is the new treasury account
    pub new_protocol_treasury: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CollectProtocolFees<'info> {
    #[account(
        seeds = [b"fee_config".as_ref()],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,
    
    #[account(
        seeds = [b"fee_vault_authority", fee_config.key().as_ref()],
        bump,
    )]
    /// CHECK: This is a PDA that serves as the authority for the fee vault
    pub fee_vault_authority: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = fee_vault.owner == fee_vault_authority.key()
    )]
    pub fee_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = protocol_treasury.key() == fee_config.protocol_treasury
    )]
    pub protocol_treasury: Account<'info, TokenAccount>,
    
    #[account(
        constraint = authority.key() == fee_config.authority
    )]
    pub authority: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CalculateFees<'info> {
    #[account(
        seeds = [b"fee_config".as_ref()],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,
    
    /// CHECK: This account is calling the calculation function
    pub caller: AccountInfo<'info>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Fee is too high")]
    FeeTooHigh,
    #[msg("Fee distribution percentages must add up to 100%")]
    FeeDistributionInvalid,
}
