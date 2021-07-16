#ifndef _CKB_SPARSE_MERKLE_TREE_H_
#define _CKB_SPARSE_MERKLE_TREE_H_

// origin from:
// https://github.com/nervosnetwork/godwoken/blob/6c9b92b9b06068a8678864b35a3272545ed7909e/c/gw_smt.h#L1
#define _SMT_STACK_SIZE 32
#define SMT_KEY_BYTES 32
#define SMT_VALUE_BYTES 32

enum SMTErrorCode {
  // SMT
  ERROR_INSUFFICIENT_CAPACITY = 80,
  ERROR_NOT_FOUND,
  ERROR_INVALID_STACK,
  ERROR_INVALID_SIBLING,
  ERROR_INVALID_PROOF
};

/* Key Value Pairs */
typedef struct {
  uint8_t key[SMT_KEY_BYTES];
  uint8_t value[SMT_VALUE_BYTES];
  uint32_t order;
} smt_pair_t;

typedef struct {
  smt_pair_t *pairs;
  uint32_t len;
  uint32_t capacity;
} smt_state_t;

void smt_state_init(smt_state_t *state, smt_pair_t *buffer, uint32_t capacity) {
  state->pairs = buffer;
  state->len = 0;
  state->capacity = capacity;
}

int smt_state_insert(smt_state_t *state, const uint8_t *key,
                     const uint8_t *value) {
  if (state->len < state->capacity) {
    /* shortcut, append at end */
    memcpy(state->pairs[state->len].key, key, SMT_KEY_BYTES);
    memcpy(state->pairs[state->len].value, value, SMT_KEY_BYTES);
    state->len++;
    return 0;
  }

  /* Find a matched key and overwritten it */
  int32_t i = state->len - 1;
  for (; i >= 0; i--) {
    if (memcmp(key, state->pairs[i].key, SMT_KEY_BYTES) == 0) {
      break;
    }
  }

  if (i < 0) {
    return ERROR_INSUFFICIENT_CAPACITY;
  }

  memcpy(state->pairs[i].value, value, SMT_VALUE_BYTES);
  return 0;
}

int smt_state_fetch(smt_state_t *state, const uint8_t *key, uint8_t *value) {
  int32_t i = state->len - 1;
  for (; i >= 0; i--) {
    if (memcmp(key, state->pairs[i].key, SMT_KEY_BYTES) == 0) {
      memcpy(value, state->pairs[i].value, SMT_VALUE_BYTES);
      return 0;
    }
  }
  return ERROR_NOT_FOUND;
}

int _smt_pair_cmp(const void *a, const void *b) {
  const smt_pair_t *pa = (const smt_pair_t *)a;
  const smt_pair_t *pb = (const smt_pair_t *)b;

  for (int i = SMT_KEY_BYTES - 1; i >= 0; i--) {
    int cmp_result = pa->key[i] - pb->key[i];
    if (cmp_result != 0) {
      return cmp_result;
    }
  }
  return pa->order - pb->order;
}

void smt_state_normalize(smt_state_t *state) {
  for (uint32_t i = 0; i < state->len; i++) {
    state->pairs[i].order = state->len - i;
  }
  qsort(state->pairs, state->len, sizeof(smt_pair_t), _smt_pair_cmp);
  /* Remove duplicate ones */
  int32_t sorted = 0, next = 0;
  while (next < (int32_t)state->len) {
    int32_t item_index = next++;
    while (next < (int32_t)state->len &&
           memcmp(state->pairs[item_index].key, state->pairs[next].key,
                  SMT_KEY_BYTES) == 0) {
      next++;
    }
    if (item_index != sorted) {
      memcpy(state->pairs[sorted].key, state->pairs[item_index].key,
             SMT_KEY_BYTES);
      memcpy(state->pairs[sorted].value, state->pairs[item_index].value,
             SMT_VALUE_BYTES);
    }
    sorted++;
  }
  state->len = sorted;
}

/* SMT */

int _smt_get_bit(const uint8_t *data, int offset) {
  int byte_pos = offset / 8;
  int bit_pos = offset % 8;
  return ((data[byte_pos] >> bit_pos) & 1) != 0;
}

void _smt_set_bit(uint8_t *data, int offset) {
  int byte_pos = offset / 8;
  int bit_pos = offset % 8;
  data[byte_pos] |= 1 << bit_pos;
}

void _smt_clear_bit(uint8_t *data, int offset) {
  int byte_pos = offset / 8;
  int bit_pos = offset % 8;
  data[byte_pos] &= (uint8_t)(~(1 << bit_pos));
}

void _smt_copy_bits(uint8_t *source, int first_kept_bit) {
  int first_byte = first_kept_bit / 8;
  for (int i = 0; i < first_byte; i++) {
    source[i] = 0;
  }
  for (int i = first_byte * 8; i < first_kept_bit; i++) {
    _smt_clear_bit(source, i);
  }
}

void _smt_parent_path(uint8_t *key, uint8_t height) {
  if (height == 255) {
    memset(key, 0, 32);
  } else {
    _smt_copy_bits(key, height + 1);
  }
}

int _smt_zero_value(const uint8_t *value) {
  for (int i = 0; i < 32; i++) {
    if (value[i] != 0) {
      return 0;
    }
  }
  return 1;
}

/* Notice that output might collide with one of lhs, or rhs */
void _smt_merge(const uint8_t *lhs, const uint8_t *rhs, uint8_t *output) {
  if (_smt_zero_value(lhs)) {
    memcpy(output, rhs, 32);
  } else if (_smt_zero_value(rhs)) {
    memcpy(output, lhs, 32);
  } else {
    blake2b_state blake2b_ctx;
    blake2b_init(&blake2b_ctx, 32);
    blake2b_update(&blake2b_ctx, lhs, 32);
    blake2b_update(&blake2b_ctx, rhs, 32);
    blake2b_final(&blake2b_ctx, output, 32);
  }
}

/*
 * Theoretically, a stack size of x should be able to process as many as
 * 2 ** (x - 1) updates. In this case with a stack size of 32, we can deal
 * with 2 ** 31 == 2147483648 updates, which is more than enough.
 */
int smt_calculate_root(uint8_t *buffer, const smt_state_t *pairs,
                       const uint8_t *proof, uint32_t proof_length) {
  blake2b_state blake2b_ctx;
  uint8_t stack_keys[_SMT_STACK_SIZE][SMT_KEY_BYTES];
  uint8_t stack_values[_SMT_STACK_SIZE][32];
  uint32_t proof_index = 0;
  uint32_t leave_index = 0;
  uint32_t stack_top = 0;

  while (proof_index < proof_length) {
    switch (proof[proof_index++]) {
      case 0x4C:
        if (stack_top >= _SMT_STACK_SIZE) {
          return ERROR_INVALID_STACK;
        }
        if (leave_index >= pairs->len) {
          return ERROR_INVALID_PROOF;
        }
        memcpy(stack_keys[stack_top], pairs->pairs[leave_index].key,
               SMT_KEY_BYTES);
        if (_smt_zero_value(pairs->pairs[leave_index].value)) {
          memset(stack_values[stack_top], 0, 32);
        } else {
          blake2b_init(&blake2b_ctx, 32);
          blake2b_update(&blake2b_ctx, pairs->pairs[leave_index].key,
                         SMT_KEY_BYTES);
          blake2b_update(&blake2b_ctx, pairs->pairs[leave_index].value,
                         SMT_KEY_BYTES);
          blake2b_final(&blake2b_ctx, stack_values[stack_top], 32);
        }
        stack_top++;
        leave_index++;
        break;
      case 0x50: {
        if (stack_top == 0) {
          return ERROR_INVALID_STACK;
        }
        if (proof_index + 33 > proof_length) {
          return ERROR_INVALID_PROOF;
        }
        uint8_t height = proof[proof_index++];
        const uint8_t *current_proof = &proof[proof_index];
        proof_index += 32;
        uint8_t *key = stack_keys[stack_top - 1];
        uint8_t *value = stack_values[stack_top - 1];
        if (_smt_get_bit(key, height)) {
          _smt_merge(current_proof, value, value);
        } else {
          _smt_merge(value, current_proof, value);
        }
        _smt_parent_path(key, height);
      } break;
      case 0x48: {
        if (stack_top < 2) {
          return ERROR_INVALID_STACK;
        }
        if (proof_index >= proof_length) {
          return ERROR_INVALID_PROOF;
        }
        uint8_t height = proof[proof_index++];
        uint8_t *key_a = stack_keys[stack_top - 2];
        uint8_t *value_a = stack_values[stack_top - 2];
        uint8_t *key_b = stack_keys[stack_top - 1];
        uint8_t *value_b = stack_values[stack_top - 1];
        stack_top -= 2;
        int a_set = _smt_get_bit(key_a, height);
        int b_set = _smt_get_bit(key_b, height);
        _smt_copy_bits(key_a, height);
        _smt_copy_bits(key_b, height);
        uint8_t sibling_key_a[32];
        memcpy(sibling_key_a, key_a, 32);
        if (!a_set) {
          _smt_set_bit(sibling_key_a, height);
        }
        if (memcmp(sibling_key_a, key_b, 32) != 0 || (a_set == b_set)) {
          return ERROR_INVALID_SIBLING;
        }
        if (a_set) {
          _smt_merge(value_b, value_a, value_a);
        } else {
          _smt_merge(value_a, value_b, value_a);
        }
        /* Top-of-stack key is already updated to parent_key_a */
        stack_top++;
      } break;
      default:
        return ERROR_INVALID_PROOF;
    }
  }
  /* All leaves must be used */
  if (leave_index != pairs->len) {
    return ERROR_INVALID_PROOF;
  }
  if (stack_top != 1) {
    return ERROR_INVALID_STACK;
  }
  memcpy(buffer, stack_values[0], 32);
  return 0;
}

int smt_verify(const uint8_t *hash, const smt_state_t *state,
               const uint8_t *proof, uint32_t proof_length) {
  uint8_t buffer[32];
  int ret = smt_calculate_root(buffer, state, proof, proof_length);
  if (ret != 0) {
    return ret;
  }
  if (memcmp(buffer, hash, 32) != 0) {
    return ERROR_INVALID_PROOF;
  }
  return 0;
}

#endif
