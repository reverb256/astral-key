'use strict';

/**
 * Mosaic Label Filter Engine
 *
 * Client-side filter that lets each user configure which labelers
 * to trust and how to handle specific label values.
 *
 * Behaviours for label values:
 *   hide  — completely hide labelled content
 *   blur  — blur until user clicks to reveal
 *   warn  — show a warning banner above content
 *   none  — ignore the label (default)
 */

class LabelFilter {
  /**
   * @param {object} [preferences]
   * @param {string[]} [preferences.trustedLabelers] — pubkeys the user trusts
   * @param {object} [preferences.labelBehaviors]    — { [labelValue]: 'hide'|'blur'|'warn'|'none' }
   */
  constructor(preferences = {}) {
    this._trustedLabelers = new Set(preferences.trustedLabelers || []);
    this._labelBehaviors = new Map(Object.entries(preferences.labelBehaviors || {}));

    // Default behaviours for well-known label values
    if (!this._labelBehaviors.has('spam'))        this._labelBehaviors.set('spam', 'hide');
    if (!this._labelBehaviors.has('harassment'))   this._labelBehaviors.set('harassment', 'hide');
    if (!this._labelBehaviors.has('nsfw'))         this._labelBehaviors.set('nsfw', 'blur');
    if (!this._labelBehaviors.has('misinfo'))      this._labelBehaviors.set('misinfo', 'warn');
  }

  // ─── Labeler management ──────────────────────────────

  addLabeler(pubkey) {
    this._trustedLabelers.add(pubkey);
  }

  removeLabeler(pubkey) {
    this._trustedLabelers.delete(pubkey);
  }

  getTrustedLabelers() {
    return [...this._trustedLabelers];
  }

  isLabelerTrusted(pubkey) {
    return this._trustedLabelers.has(pubkey);
  }

  // ─── Label behaviour configuration ───────────────────

  getLabelBehavior(val) {
    return this._labelBehaviors.get(val) || 'none';
  }

  setLabelBehavior(val, behavior) {
    if (!['hide', 'blur', 'warn', 'none'].includes(behavior)) {
      throw new Error(`Invalid behavior: ${behavior}. Must be hide, blur, warn, or none.`);
    }
    this._labelBehaviors.set(val, behavior);
  }

  // ─── Filtering ───────────────────────────────────────

  /**
   * Filter an array of items based on their labels.
   * Each item should have a `uri` and optionally `labels` array.
   * Items that should be hidden are removed from the result.
   *
   * @param {object[]} items — items with { uri, labels?, ... }
   * @returns {{ visible: object[], blurred: object[], warned: object[] }}
   */
  filter(items) {
    const visible = [];
    const blurred = [];
    const warned = [];

    for (const item of items) {
      const labels = item.labels || [];
      const activeLabels = labels.filter(l => this.isActiveLabel(l));

      // Group labels by their most severe behaviour
      let maxBehaviour = 'none';
      for (const l of activeLabels) {
        const behavior = this.getLabelBehavior(l.val);
        if (behavior === 'hide')   { maxBehaviour = 'hide'; break; }
        if (behavior === 'blur' && maxBehaviour !== 'hide')   maxBehaviour = 'blur';
        if (behavior === 'warn' && !['hide', 'blur'].includes(maxBehaviour)) maxBehaviour = 'warn';
      }

      switch (maxBehaviour) {
        case 'hide':
          // Skip — item is hidden
          break;
        case 'blur':
          blurred.push(item);
          break;
        case 'warn':
          warned.push(item);
          break;
        default:
          visible.push(item);
      }
    }

    return { visible, blurred, warned };
  }

  /**
   * Check whether a specific URI should be hidden.
   */
  shouldHide(uri) {
    // This is a stub — the caller should pass labels context.
    // Full implementation needs label lookups.
    return false;
  }

  /**
   * Check whether a specific URI should be blurred.
   */
  shouldBlur(uri) {
    return false;
  }

  // ─── Helpers ─────────────────────────────────────────

  isActiveLabel(label) {
    // Label must be from a trusted labeler, non-negated, and not expired
    if (!this._trustedLabelers.has(label.src)) return false;
    if (label.neg) return false;
    if (label.expires_at && new Date(label.expires_at) < new Date()) return false;
    return true;
  }

  /**
   * Serialise the current configuration for storage
   * (e.g. in localStorage or user_preferences).
   */
  toJSON() {
    return {
      trustedLabelers: [...this._trustedLabelers],
      labelBehaviors: Object.fromEntries(this._labelBehaviors),
    };
  }

  /**
   * Restore from a previously serialised config.
   */
  static fromJSON(json) {
    return new LabelFilter(json);
  }
}

module.exports = LabelFilter;
