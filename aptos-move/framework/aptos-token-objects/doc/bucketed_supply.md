
<a name="0x4_bucketed_supply"></a>

# Module `0x4::bucketed_supply`



-  [Resource `SingleBucketSupply`](#0x4_bucketed_supply_SingleBucketSupply)
-  [Resource `BucketedSupplyCounter`](#0x4_bucketed_supply_BucketedSupplyCounter)
-  [Function `create_bucketed_supply`](#0x4_bucketed_supply_create_bucketed_supply)
-  [Function `get_txn_bucket`](#0x4_bucketed_supply_get_txn_bucket)
-  [Function `try_increment`](#0x4_bucketed_supply_try_increment)
-  [Function `decrement`](#0x4_bucketed_supply_decrement)


<pre><code><b>use</b> <a href="../../aptos-framework/doc/object.md#0x1_object">0x1::object</a>;
<b>use</b> <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../../aptos-framework/doc/transaction_context.md#0x1_transaction_context">0x1::transaction_context</a>;
</code></pre>



<a name="0x4_bucketed_supply_SingleBucketSupply"></a>

## Resource `SingleBucketSupply`



<pre><code>#[resource_group_member(#[group = <a href="../../aptos-framework/doc/object.md#0x1_object_ObjectGroup">0x1::object::ObjectGroup</a>])]
<b>struct</b> <a href="bucketed_supply.md#0x4_bucketed_supply_SingleBucketSupply">SingleBucketSupply</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>current_supply: u64</code>
</dt>
<dd>

</dd>
<dt>
<code>total_supply: u64</code>
</dt>
<dd>

</dd>
<dt>
<code>max_active_supply: u64</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="0x4_bucketed_supply_BucketedSupplyCounter"></a>

## Resource `BucketedSupplyCounter`



<pre><code>#[resource_group_member(#[group = <a href="../../aptos-framework/doc/object.md#0x1_object_ObjectGroup">0x1::object::ObjectGroup</a>])]
<b>struct</b> <a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>buckets: <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;<a href="../../aptos-framework/doc/object.md#0x1_object_Object">object::Object</a>&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_SingleBucketSupply">bucketed_supply::SingleBucketSupply</a>&gt;&gt;</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="0x4_bucketed_supply_create_bucketed_supply"></a>

## Function `create_bucketed_supply`



<pre><code><b>public</b> <b>fun</b> <a href="bucketed_supply.md#0x4_bucketed_supply_create_bucketed_supply">create_bucketed_supply</a>(creator: <b>address</b>, max_supply: u64, num_buckets: u64): <a href="../../aptos-framework/doc/object.md#0x1_object_Object">object::Object</a>&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">bucketed_supply::BucketedSupplyCounter</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="bucketed_supply.md#0x4_bucketed_supply_create_bucketed_supply">create_bucketed_supply</a>(
    creator: <b>address</b>,
    max_supply: u64,
    num_buckets: u64,
): Object&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a>&gt; {
    <b>let</b> buckets = <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_empty">vector::empty</a>();

    <b>let</b> i = 0;
    <b>while</b> (i &lt; num_buckets) {
        <b>let</b> constructor_ref = <a href="../../aptos-framework/doc/object.md#0x1_object_create_object">object::create_object</a>(creator);
        <b>let</b> object_signer = <a href="../../aptos-framework/doc/object.md#0x1_object_generate_signer">object::generate_signer</a>(&constructor_ref);
        <b>move_to</b>(&object_signer, <a href="bucketed_supply.md#0x4_bucketed_supply_SingleBucketSupply">SingleBucketSupply</a> {
            current_supply: 0,
            total_supply: 0,
            max_active_supply: (max_supply + i) / num_buckets,
        });

        <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_push_back">vector::push_back</a>(&<b>mut</b> buckets, <a href="../../aptos-framework/doc/object.md#0x1_object_object_from_constructor_ref">object::object_from_constructor_ref</a>(&constructor_ref));
        i = i + 1;
    };

    <b>let</b> constructor_ref = <a href="../../aptos-framework/doc/object.md#0x1_object_create_object">object::create_object</a>(creator);
    <b>let</b> object_signer = <a href="../../aptos-framework/doc/object.md#0x1_object_generate_signer">object::generate_signer</a>(&constructor_ref);
    <b>move_to</b>(&object_signer, <a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a> {
        buckets: buckets,
    });
    <a href="../../aptos-framework/doc/object.md#0x1_object_object_from_constructor_ref">object::object_from_constructor_ref</a>(&constructor_ref)
}
</code></pre>



</details>

<a name="0x4_bucketed_supply_get_txn_bucket"></a>

## Function `get_txn_bucket`



<pre><code><b>fun</b> <a href="bucketed_supply.md#0x4_bucketed_supply_get_txn_bucket">get_txn_bucket</a>(num_buckets: u64): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="bucketed_supply.md#0x4_bucketed_supply_get_txn_bucket">get_txn_bucket</a>(num_buckets: u64): u64 {
    <b>let</b> txn_hash = <a href="../../aptos-framework/doc/transaction_context.md#0x1_transaction_context_get_transaction_hash">transaction_context::get_transaction_hash</a>();
    ((*<a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>(&txn_hash, 0) <b>as</b> u64) * 256u64 + (*<a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>(&txn_hash, 1) <b>as</b> u64)) % num_buckets
}
</code></pre>



</details>

<a name="0x4_bucketed_supply_try_increment"></a>

## Function `try_increment`



<pre><code><b>public</b> <b>fun</b> <a href="bucketed_supply.md#0x4_bucketed_supply_try_increment">try_increment</a>(supply_obj: &<b>mut</b> <a href="../../aptos-framework/doc/object.md#0x1_object_Object">object::Object</a>&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">bucketed_supply::BucketedSupplyCounter</a>&gt;): <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/option.md#0x1_option_Option">option::Option</a>&lt;u64&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="bucketed_supply.md#0x4_bucketed_supply_try_increment">try_increment</a>(
    supply_obj: &<b>mut</b> Object&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a>&gt;,
): Option&lt;u64&gt; <b>acquires</b> <a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a>, <a href="bucketed_supply.md#0x4_bucketed_supply_SingleBucketSupply">SingleBucketSupply</a> {
    <b>let</b> supply = <b>borrow_global</b>&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a>&gt;(<a href="../../aptos-framework/doc/object.md#0x1_object_object_address">object::object_address</a>(supply_obj));

    <b>let</b> num_buckets = <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&supply.buckets);
    <b>let</b> start = <a href="bucketed_supply.md#0x4_bucketed_supply_get_txn_bucket">get_txn_bucket</a>(num_buckets);

    <b>let</b> offset = 0;
    <b>while</b> (offset &lt; num_buckets) {
        <b>let</b> cur_bucket_index = (start + offset) % num_buckets;

        <b>let</b> cur_bucket_obj = <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>(&supply.buckets, cur_bucket_index);
        <b>let</b> cur_bucket = <b>borrow_global_mut</b>&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_SingleBucketSupply">SingleBucketSupply</a>&gt;(<a href="../../aptos-framework/doc/object.md#0x1_object_object_address">object::object_address</a>(cur_bucket_obj));

        <b>if</b> (cur_bucket.current_supply &lt; cur_bucket.max_active_supply) {
            cur_bucket.current_supply = cur_bucket.current_supply + 1;
            cur_bucket.total_supply = cur_bucket.total_supply + 1;
            <b>return</b> <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/option.md#0x1_option_some">option::some</a>(cur_bucket.total_supply * num_buckets + cur_bucket_index)
        };

        offset = offset + 1;
    };

    <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/option.md#0x1_option_none">option::none</a>()
}
</code></pre>



</details>

<a name="0x4_bucketed_supply_decrement"></a>

## Function `decrement`



<pre><code><b>public</b> <b>fun</b> <a href="bucketed_supply.md#0x4_bucketed_supply_decrement">decrement</a>(supply_obj: &<b>mut</b> <a href="../../aptos-framework/doc/object.md#0x1_object_Object">object::Object</a>&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">bucketed_supply::BucketedSupplyCounter</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="bucketed_supply.md#0x4_bucketed_supply_decrement">decrement</a>(
    supply_obj: &<b>mut</b> Object&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a>&gt;,
) <b>acquires</b> <a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a>, <a href="bucketed_supply.md#0x4_bucketed_supply_SingleBucketSupply">SingleBucketSupply</a> {
    <b>let</b> supply = <b>borrow_global_mut</b>&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_BucketedSupplyCounter">BucketedSupplyCounter</a>&gt;(<a href="../../aptos-framework/doc/object.md#0x1_object_object_address">object::object_address</a>(supply_obj));

    <b>let</b> num_buckets = <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&supply.buckets);
    <b>let</b> start = <a href="bucketed_supply.md#0x4_bucketed_supply_get_txn_bucket">get_txn_bucket</a>(num_buckets);

    <b>let</b> offset = 0;
    <b>while</b> (offset &lt; num_buckets) {
        <b>let</b> cur_bucket_index = (start + offset) % num_buckets;

        <b>let</b> cur_bucket_obj = <a href="../../aptos-framework/../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow_mut">vector::borrow_mut</a>(&<b>mut</b> supply.buckets, cur_bucket_index);
        <b>let</b> cur_bucket = <b>borrow_global_mut</b>&lt;<a href="bucketed_supply.md#0x4_bucketed_supply_SingleBucketSupply">SingleBucketSupply</a>&gt;(<a href="../../aptos-framework/doc/object.md#0x1_object_object_address">object::object_address</a>(cur_bucket_obj));

        <b>if</b> (cur_bucket.current_supply &gt; 0) {
            cur_bucket.current_supply = cur_bucket.current_supply - 1;
            <b>return</b>
        };

        offset = offset + 1;
    };
}
</code></pre>



</details>


[move-book]: https://aptos.dev/move/book/SUMMARY
