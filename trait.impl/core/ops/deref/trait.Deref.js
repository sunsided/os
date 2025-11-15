(function() {
    var implementors = Object.fromEntries([["kernel",[["impl&lt;K&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a> for <a class=\"struct\" href=\"kernel/gdt/selectors/struct.SegmentSelector.html\" title=\"struct kernel::gdt::selectors::SegmentSelector\">SegmentSelector</a>&lt;K&gt;<div class=\"where\">where\n    K: <a class=\"trait\" href=\"kernel/gdt/selectors/trait.SelectorKind.html\" title=\"trait kernel::gdt::selectors::SelectorKind\">SelectorKind</a>,</div>"]]],["kernel_sync",[["impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a> for <a class=\"struct\" href=\"kernel_sync/struct.SpinLockGuard.html\" title=\"struct kernel_sync::SpinLockGuard\">SpinLockGuard</a>&lt;'_, T&gt;"],["impl&lt;T, R&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a> for <a class=\"struct\" href=\"kernel_sync/struct.MutexGuard.html\" title=\"struct kernel_sync::MutexGuard\">MutexGuard</a>&lt;'_, T, R&gt;<div class=\"where\">where\n    R: <a class=\"trait\" href=\"kernel_sync/trait.RawUnlock.html\" title=\"trait kernel_sync::RawUnlock\">RawUnlock</a>,</div>"]]]]);
    if (window.register_implementors) {
        window.register_implementors(implementors);
    } else {
        window.pending_implementors = implementors;
    }
})()
//{"start":57,"fragment_lengths":[526,786]}