// Verifies at compile time that the discriminants of `MiOption` in `src/options.rs`
// match `mi_option_t` of the bundled mimalloc version.

#include "mimalloc.h"

// Compiled as C++ with MSVC (see build.rs).
#ifdef __cplusplus
#define MI_OPTION_CHECK static_assert
#else
#define MI_OPTION_CHECK _Static_assert
#endif

MI_OPTION_CHECK(mi_option_show_errors == 0, "MiOption::ShowErrors does not match mi_option_show_errors");
MI_OPTION_CHECK(mi_option_show_stats == 1, "MiOption::ShowStats does not match mi_option_show_stats");
MI_OPTION_CHECK(mi_option_verbose == 2, "MiOption::Verbose does not match mi_option_verbose");
MI_OPTION_CHECK(mi_option_eager_commit == 3, "MiOption::EagerCommit does not match mi_option_eager_commit");
MI_OPTION_CHECK(mi_option_arena_eager_commit == 4, "MiOption::ArenaEagerCommit does not match mi_option_arena_eager_commit");
MI_OPTION_CHECK(mi_option_purge_decommits == 5, "MiOption::PurgeDecommits does not match mi_option_purge_decommits");
MI_OPTION_CHECK(mi_option_allow_large_os_pages == 6, "MiOption::AllowLargeOsPages does not match mi_option_allow_large_os_pages");
MI_OPTION_CHECK(mi_option_reserve_huge_os_pages == 7, "MiOption::ReserveHugeOsPages does not match mi_option_reserve_huge_os_pages");
MI_OPTION_CHECK(mi_option_reserve_huge_os_pages_at == 8, "MiOption::ReserveHugeOsPagesAt does not match mi_option_reserve_huge_os_pages_at");
MI_OPTION_CHECK(mi_option_reserve_os_memory == 9, "MiOption::ReserveOsMemory does not match mi_option_reserve_os_memory");
MI_OPTION_CHECK(mi_option_abandoned_page_purge == 12, "MiOption::AbandonedPagePurge does not match mi_option_abandoned_page_purge");
MI_OPTION_CHECK(mi_option_eager_commit_delay == 14, "MiOption::EagerCommitDelay does not match mi_option_eager_commit_delay");
MI_OPTION_CHECK(mi_option_purge_delay == 15, "MiOption::PurgeDelay does not match mi_option_purge_delay");
MI_OPTION_CHECK(mi_option_use_numa_nodes == 16, "MiOption::UseNumaNodes does not match mi_option_use_numa_nodes");
MI_OPTION_CHECK(mi_option_disallow_os_alloc == 17, "MiOption::DisallowOsAlloc does not match mi_option_disallow_os_alloc");
MI_OPTION_CHECK(mi_option_os_tag == 18, "MiOption::OsTag does not match mi_option_os_tag");
MI_OPTION_CHECK(mi_option_max_errors == 19, "MiOption::MaxErrors does not match mi_option_max_errors");
MI_OPTION_CHECK(mi_option_max_warnings == 20, "MiOption::MaxWarnings does not match mi_option_max_warnings");
MI_OPTION_CHECK(mi_option_max_segment_reclaim == 21, "MiOption::MaxSegmentReclaim does not match mi_option_max_segment_reclaim");
MI_OPTION_CHECK(mi_option_destroy_on_exit == 22, "MiOption::DestroyOnExit does not match mi_option_destroy_on_exit");
MI_OPTION_CHECK(mi_option_arena_reserve == 23, "MiOption::ArenaReserve does not match mi_option_arena_reserve");
MI_OPTION_CHECK(mi_option_arena_purge_mult == 24, "MiOption::ArenaPurgeMult does not match mi_option_arena_purge_mult");
MI_OPTION_CHECK(mi_option_purge_extend_delay == 25, "MiOption::PurgeExtendDelay does not match mi_option_purge_extend_delay");
MI_OPTION_CHECK(mi_option_disallow_arena_alloc == 26, "MiOption::DisallowArenaAlloc does not match mi_option_disallow_arena_alloc");
MI_OPTION_CHECK(mi_option_retry_on_oom == 27, "MiOption::RetryOnOom does not match mi_option_retry_on_oom");
MI_OPTION_CHECK(mi_option_visit_abandoned == 28, "MiOption::VisitAbandoned does not match mi_option_visit_abandoned");
MI_OPTION_CHECK(mi_option_guarded_min == 29, "MiOption::GuardedMin does not match mi_option_guarded_min");
MI_OPTION_CHECK(mi_option_guarded_max == 30, "MiOption::GuardedMax does not match mi_option_guarded_max");
MI_OPTION_CHECK(mi_option_guarded_precise == 31, "MiOption::GuardedPrecise does not match mi_option_guarded_precise");
MI_OPTION_CHECK(mi_option_guarded_sample_rate == 32, "MiOption::GuardedSampleRate does not match mi_option_guarded_sample_rate");
MI_OPTION_CHECK(mi_option_guarded_sample_seed == 33, "MiOption::GuardedSampleSeed does not match mi_option_guarded_sample_seed");
MI_OPTION_CHECK(mi_option_target_segments_per_thread == 34, "MiOption::TargetSegmentsPerThread does not match mi_option_target_segments_per_thread");
MI_OPTION_CHECK(mi_option_reclaim_on_free == 35, "MiOption::ReclaimOnFree does not match mi_option_reclaim_on_free");
MI_OPTION_CHECK(mi_option_page_full_retain == 36, "MiOption::PageFullRetain does not match mi_option_page_full_retain");
MI_OPTION_CHECK(mi_option_page_max_candidates == 37, "MiOption::PageMaxCandidates does not match mi_option_page_max_candidates");
MI_OPTION_CHECK(mi_option_max_vabits == 38, "MiOption::MaxVabits does not match mi_option_max_vabits");
MI_OPTION_CHECK(mi_option_pagemap_commit == 39, "MiOption::PagemapCommit does not match mi_option_pagemap_commit");
MI_OPTION_CHECK(mi_option_page_commit_on_demand == 40, "MiOption::PageCommitOnDemand does not match mi_option_page_commit_on_demand");
MI_OPTION_CHECK(_mi_option_last == 41, "mi_option_t has new options, update MiOption");
