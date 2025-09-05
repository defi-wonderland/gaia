// @generated
pub mod sf {
    pub mod firehose {
        // @@protoc_insertion_point(attribute:sf.firehose.v2)
        pub mod v2 {
            include!("sf.firehose.v2.rs");
            // @@protoc_insertion_point(sf.firehose.v2)
        }
    }
    // @@protoc_insertion_point(attribute:sf.substreams)
    pub mod substreams {
        pub mod rpc {
            // @@protoc_insertion_point(attribute:sf.substreams.rpc.v2)
            pub mod v2 {
                include!("sf.substreams.rpc.v2.rs");
                // @@protoc_insertion_point(sf.substreams.rpc.v2)
            }
        }
        pub mod v1 {
            include!("sf.substreams.v1.rs");
            // @@protoc_insertion_point(sf.substreams.v1)
        }
    }

    pub mod actions {
        // @@protoc_insertion_point(attribute:actions.v1)
        pub mod v1 {
            include!("actions.v1.rs");
            // @@protoc_insertion_point(actions.v1)
        }
    }

}

