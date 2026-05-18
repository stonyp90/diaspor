/**
 * Token Management Service
 *
 * Manages user tokens for AI features:
 * - Free tier: X tokens per month
 * - Paid tier: Unlimited tokens
 * - Token consumption tracking
 * - Payment integration
 */

import { invoke } from '@tauri-apps/api/core';

export interface TokenBalance {
  total: number;
  used: number;
  remaining: number;
  reset_date: string; // ISO 8601 date string from backend
  is_paid: boolean;
  plan_id: string;
}

// Frontend-friendly version with Date object
export interface TokenBalanceUI {
  total: number;
  used: number;
  remaining: number;
  resetDate: Date;
  isPaid: boolean;
  planId: string;
}

export interface TokenUsage {
  operation: string;
  tokensUsed: number;
  timestamp: Date;
  description?: string;
}

export interface TokenPlan {
  id: string;
  name: string;
  price: number;
  tokens_per_month: number; // Backend uses snake_case
  features: string[];
}

// Frontend-friendly version
export interface TokenPlanUI {
  id: string;
  name: string;
  price: number;
  tokensPerMonth: number;
  features: string[];
}

export class TokenService {
  private static readonly FREE_TOKENS_PER_MONTH = 1000;
  private static readonly STORAGE_KEY = 'diaspor_token_balance';

  /**
   * Get current token balance
   */
  static async getBalance(): Promise<TokenBalance> {
    try {
      // Try to get from backend first (for paid users)
      const backendBalance = await invoke<TokenBalance | null>(
        'get_token_balance',
      );

      if (backendBalance) {
        return backendBalance;
      }

      // Fallback to local storage for free tier
      const stored = localStorage.getItem(this.STORAGE_KEY);
      if (stored) {
        const parsed = JSON.parse(stored);
        // Ensure we return TokenBalance format (snake_case)
        return {
          total: parsed.total,
          used: parsed.used,
          remaining: parsed.remaining,
          reset_date:
            parsed.reset_date || parsed.resetDate || new Date().toISOString(),
          is_paid: parsed.is_paid || parsed.isPaid || false,
          plan_id: parsed.plan_id || parsed.planId || 'free',
        };
      }

      // Initialize new free tier
      const now = new Date();
      const resetDate = new Date(now.getFullYear(), now.getMonth() + 1, 1);

      const initialBalance: TokenBalance = {
        total: this.FREE_TOKENS_PER_MONTH,
        used: 0,
        remaining: this.FREE_TOKENS_PER_MONTH,
        reset_date: resetDate.toISOString(),
        is_paid: false,
        plan_id: 'free',
      };

      localStorage.setItem(this.STORAGE_KEY, JSON.stringify(initialBalance));
      return initialBalance;
    } catch (error) {
      console.error('Failed to get token balance:', error);
      // Return default free tier on error
      return {
        total: this.FREE_TOKENS_PER_MONTH,
        used: 0,
        remaining: this.FREE_TOKENS_PER_MONTH,
        reset_date: new Date().toISOString(),
        is_paid: false,
        plan_id: 'free',
      };
    }
  }

  /**
   * Check if user has enough tokens for an operation
   */
  static async canPerformOperation(tokensRequired: number): Promise<boolean> {
    const balance = await this.getBalance();

    // Unlimited plan
    if (balance.total === -1) {
      return true;
    }

    // Check if reset date has passed (backend handles this automatically)
    return balance.remaining >= tokensRequired;
  }

  /**
   * Consume tokens for an operation
   */
  static async consumeTokens(
    tokens: number,
    operation: string,
    description?: string,
  ): Promise<boolean> {
    const canPerform = await this.canPerformOperation(tokens);
    if (!canPerform) {
      return false;
    }

    try {
      // Backend handles all token consumption and persistence
      const success = await invoke<boolean>('consume_tokens', {
        tokens,
        operation,
        description,
      });

      if (success) {
        // Log usage locally for UI display
        const usage: TokenUsage = {
          operation,
          tokensUsed: tokens,
          timestamp: new Date(),
          description,
        };

        const usageHistory = this.getUsageHistory();
        usageHistory.push(usage);

        // Keep only last 100 entries
        if (usageHistory.length > 100) {
          usageHistory.shift();
        }

        localStorage.setItem('diaspor_token_usage', JSON.stringify(usageHistory));

        return true;
      }

      return false;
    } catch (error) {
      console.error('Failed to consume tokens:', error);
      return false;
    }
  }

  /**
   * Reset monthly tokens (backend handles this automatically)
   * This method is kept for backward compatibility but does nothing
   */
  static async resetMonthlyTokens(): Promise<void> {
    // Backend automatically resets tokens when reset_date passes
    // No local action needed
  }

  /**
   * Get usage history
   */
  static getUsageHistory(): TokenUsage[] {
    try {
      const stored = localStorage.getItem('diaspor_token_usage');
      if (stored) {
        const parsed = JSON.parse(stored) as Array<{
          operation: string;
          tokensUsed: number;
          timestamp: string;
          description?: string;
        }>;
        return parsed.map((u) => ({
          ...u,
          timestamp: new Date(u.timestamp),
        }));
      }
    } catch (error) {
      console.error('Failed to get usage history:', error);
    }
    return [];
  }

  /**
   * Convert backend TokenPlan to UI-friendly format
   */
  private static planToUI(plan: TokenPlan): TokenPlanUI {
    return {
      id: plan.id,
      name: plan.name,
      price: plan.price,
      tokensPerMonth: plan.tokens_per_month,
      features: plan.features,
    };
  }

  /**
   * Get available plans
   */
  static async getPlans(): Promise<TokenPlanUI[]> {
    try {
      const plans = await invoke<TokenPlan[]>('get_token_plans');
      return plans.map((p) => this.planToUI(p));
    } catch (error) {
      console.error('Failed to get plans:', error);
      // Return default plans if backend unavailable
      return [
        {
          id: 'free',
          name: 'Free',
          price: 0,
          tokensPerMonth: this.FREE_TOKENS_PER_MONTH,
          features: [
            '1,000 tokens/month',
            'Basic AI features',
            'Community support',
          ],
        },
        {
          id: 'pro',
          name: 'Pro',
          price: 9.99,
          tokensPerMonth: 10000,
          features: [
            '10,000 tokens/month',
            'All AI features',
            'Priority support',
            'Advanced transcription',
          ],
        },
        {
          id: 'unlimited',
          name: 'Unlimited',
          price: 29.99,
          tokensPerMonth: -1, // -1 means unlimited
          features: [
            'Unlimited tokens',
            'All AI features',
            'Priority support',
            'Advanced transcription',
            'Custom models',
          ],
        },
      ];
    }
  }

  /**
   * Purchase a plan (upgrade subscription)
   */
  static async purchasePlan(planId: string): Promise<boolean> {
    try {
      const success = await invoke<boolean>('purchase_token_plan', {
        plan_id: planId,
      });
      if (success) {
        // Balance is automatically updated by backend
        return true;
      }
    } catch (error) {
      console.error('Failed to purchase plan:', error);
    }
    return false;
  }

  /**
   * Get token cost for an operation
   */
  static getTokenCost(operation: string): number {
    const costs: Record<string, number> = {
      transcription: 10, // 10 tokens per minute of audio
      video_tagging: 5, // 5 tokens per video
      ai_search: 2, // 2 tokens per search
      tag_suggestion: 1, // 1 token per suggestion
    };

    return costs[operation] || 1;
  }
}
