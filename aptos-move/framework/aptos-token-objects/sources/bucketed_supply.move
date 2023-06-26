


module aptos_token_objects::bucketed_supply {
    use std::option::{Self, Option};
    use std::vector;
    use aptos_framework::object::{Self, Object};
    use aptos_framework::transaction_context;

    #[resource_group_member(group = aptos_framework::object::ObjectGroup)]
    struct SingleBucketSupply has key {
        current_supply: u64,
        total_supply: u64,
        max_active_supply: u64,
    }

    #[resource_group_member(group = aptos_framework::object::ObjectGroup)]
    struct BucketedSupplyCounter has key {
        buckets: vector<Object<SingleBucketSupply>>,
    }

    public fun create_bucketed_supply(
        creator: address,
        max_supply: u64,
        num_buckets: u64,
    ): Object<BucketedSupplyCounter> {
        let buckets = vector::empty();

        let i = 0;
        while (i < num_buckets) {
            let constructor_ref = object::create_object(creator);
            let object_signer = object::generate_signer(&constructor_ref);
            move_to(&object_signer, SingleBucketSupply {
                current_supply: 0,
                total_supply: 0,
                max_active_supply: (max_supply + i) / num_buckets,
            });

            vector::push_back(&mut buckets, object::object_from_constructor_ref(&constructor_ref));
            i = i + 1;
        };

        let constructor_ref = object::create_object(creator);
        let object_signer = object::generate_signer(&constructor_ref);
        move_to(&object_signer, BucketedSupplyCounter {
            buckets: buckets,
        });
        object::object_from_constructor_ref(&constructor_ref)
    }

    fun get_txn_bucket(num_buckets: u64): u64 {
        let txn_hash = transaction_context::get_transaction_hash();
        ((*vector::borrow(&txn_hash, 0) as u64) * 256u64 + (*vector::borrow(&txn_hash, 1) as u64)) % num_buckets
    }

    public fun try_increment(
        supply_obj: &mut Object<BucketedSupplyCounter>,
    ): Option<u64> acquires BucketedSupplyCounter, SingleBucketSupply {
        let supply = borrow_global<BucketedSupplyCounter>(object::object_address(supply_obj));

        let num_buckets = vector::length(&supply.buckets);
        let start = get_txn_bucket(num_buckets);

        let offset = 0;
        while (offset < num_buckets) {
            let cur_bucket_index = (start + offset) % num_buckets;

            let cur_bucket_obj = vector::borrow(&supply.buckets, cur_bucket_index);
            let cur_bucket = borrow_global_mut<SingleBucketSupply>(object::object_address(cur_bucket_obj));

            if (cur_bucket.current_supply < cur_bucket.max_active_supply) {
                cur_bucket.current_supply = cur_bucket.current_supply + 1;
                cur_bucket.total_supply = cur_bucket.total_supply + 1;
                return option::some(cur_bucket.total_supply * num_buckets + cur_bucket_index)
            };

            offset = offset + 1;
        };

        option::none()
    }

    public fun decrement(
        supply_obj: &mut Object<BucketedSupplyCounter>,
    ) acquires BucketedSupplyCounter, SingleBucketSupply {
        let supply = borrow_global_mut<BucketedSupplyCounter>(object::object_address(supply_obj));

        let num_buckets = vector::length(&supply.buckets);
        let start = get_txn_bucket(num_buckets);

        let offset = 0;
        while (offset < num_buckets) {
            let cur_bucket_index = (start + offset) % num_buckets;

            let cur_bucket_obj = vector::borrow_mut(&mut supply.buckets, cur_bucket_index);
            let cur_bucket = borrow_global_mut<SingleBucketSupply>(object::object_address(cur_bucket_obj));

            if (cur_bucket.current_supply > 0) {
                cur_bucket.current_supply = cur_bucket.current_supply - 1;
                return
            };

            offset = offset + 1;
        };
    }
}
