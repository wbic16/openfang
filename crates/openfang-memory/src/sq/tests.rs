//! Tests for the SQ integration module.

use super::*;

#[cfg(test)]
mod coordinate_tests {
    use super::coordinate::*;

    #[test]
    fn test_triad_display() {
        let t = Triad::new(1, 2, 3);
        assert_eq!(t.to_string(), "1.2.3");
    }

    #[test]
    fn test_triad_parse() {
        let t: Triad = "4.5.6".parse().unwrap();
        assert_eq!(t.a, 4);
        assert_eq!(t.b, 5);
        assert_eq!(t.c, 6);
    }

    #[test]
    fn test_coordinate_display() {
        let c = PhextCoordinate::new(
            Triad::new(1, 2, 3),
            Triad::new(4, 5, 6),
            Triad::new(7, 8, 9),
        );
        assert_eq!(c.to_string(), "1.2.3/4.5.6/7.8.9");
    }

    #[test]
    fn test_coordinate_parse() {
        let c: PhextCoordinate = "1.2.3/4.5.6/7.8.9".parse().unwrap();
        assert_eq!(c.z.a, 1);
        assert_eq!(c.z.b, 2);
        assert_eq!(c.z.c, 3);
        assert_eq!(c.y.a, 4);
        assert_eq!(c.y.b, 5);
        assert_eq!(c.y.c, 6);
        assert_eq!(c.x.a, 7);
        assert_eq!(c.x.b, 8);
        assert_eq!(c.x.c, 9);
    }

    #[test]
    fn test_special_coordinates() {
        assert_eq!(PhextCoordinate::origin().to_string(), "1.1.1/1.1.1/1.1.1");
        assert_eq!(PhextCoordinate::pi().to_string(), "3.1.4/1.5.9/2.6.5");
        assert_eq!(PhextCoordinate::boundary().to_string(), "9.9.9/9.9.9/9.9.9");
    }

    #[test]
    fn test_to_from_array() {
        let c = PhextCoordinate::pi();
        let arr = c.to_array();
        assert_eq!(arr, [3, 1, 4, 1, 5, 9, 2, 6, 5]);
        
        let c2 = PhextCoordinate::from_array(arr);
        assert_eq!(c, c2);
    }

    #[test]
    fn test_next_scroll_overflow() {
        let mut c = PhextCoordinate::new(
            Triad::new(1, 1, 1),
            Triad::new(1, 1, 1),
            Triad::new(1, 1, 9), // At max scroll
        );
        c.next_scroll();
        assert_eq!(c.to_string(), "1.1.1/1.1.1/1.2.1"); // Section incremented, scroll reset
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::protocol::*;
    use super::coordinate::PhextCoordinate;

    #[test]
    fn test_command_parse() {
        assert_eq!(SqCommand::parse("read"), SqCommand::Read);
        assert_eq!(SqCommand::parse("WRITE"), SqCommand::Write);
        assert_eq!(SqCommand::parse("Get"), SqCommand::Read);
        assert!(matches!(SqCommand::parse("unknown"), SqCommand::Custom(_)));
    }

    #[test]
    fn test_command_is_mutation() {
        assert!(!SqCommand::Read.is_mutation());
        assert!(SqCommand::Write.is_mutation());
        assert!(SqCommand::Append.is_mutation());
        assert!(SqCommand::Delete.is_mutation());
        assert!(!SqCommand::Toc.is_mutation());
    }

    #[test]
    fn test_request_roundtrip() {
        let coord = PhextCoordinate::pi();
        let req = SqRequest::write(coord, "Test content");
        
        let encoded = req.encode();
        let decoded = SqRequest::decode(&encoded).unwrap();
        
        assert_eq!(decoded.command, SqCommand::Write);
        assert_eq!(decoded.coordinate, coord);
        assert_eq!(decoded.content, "Test content");
    }

    #[test]
    fn test_response_roundtrip() {
        let resp = SqResponse::success("Hello, phext!");
        let encoded = resp.encode();
        let decoded = SqResponse::decode(&encoded).unwrap();
        
        assert_eq!(decoded.content, "Hello, phext!");
        assert!(decoded.success);
    }

    #[test]
    fn test_request_length_prefix() {
        let req = SqRequest::read(PhextCoordinate::origin());
        let encoded = req.encode();
        
        // First 20 bytes should be the length prefix
        let length_prefix = std::str::from_utf8(&encoded[..20]).unwrap();
        assert!(length_prefix.chars().all(|c| c.is_ascii_digit()));
        
        let length: usize = length_prefix.trim_start_matches('0').parse().unwrap();
        assert_eq!(encoded.len(), 20 + length);
    }

    #[test]
    fn test_decode_too_short() {
        let result = SqRequest::decode(b"short");
        assert!(matches!(result, Err(ProtocolError::TooShort)));
    }
}

#[cfg(test)]
mod client_tests {
    use super::client::*;
    use super::coordinate::PhextCoordinate;

    #[test]
    fn test_coordinate_allocator_agents() {
        let mut alloc = CoordinateAllocator::agents();
        
        // First allocation
        let c1 = alloc.allocate();
        assert_eq!(c1.z.a, 1); // Library 1 for agents
        assert_eq!(c1.to_string(), "1.1.1/1.1.1/1.1.1");
        
        // Second allocation
        let c2 = alloc.allocate();
        assert_eq!(c2.to_string(), "1.1.1/1.1.1/1.1.2");
    }

    #[test]
    fn test_coordinate_allocator_sessions() {
        let alloc = CoordinateAllocator::sessions();
        assert_eq!(alloc.base().z.a, 2); // Library 2 for sessions
    }

    #[test]
    fn test_coordinate_allocator_namespaces() {
        assert_eq!(CoordinateAllocator::agents().base().z.a, 1);
        assert_eq!(CoordinateAllocator::sessions().base().z.a, 2);
        assert_eq!(CoordinateAllocator::knowledge().base().z.a, 3);
        assert_eq!(CoordinateAllocator::semantic().base().z.a, 4);
        assert_eq!(CoordinateAllocator::usage().base().z.a, 5);
    }

    #[test]
    fn test_agent_coordinate_mapping() {
        // Agent 0 should be at the base
        let c0 = CoordinateAllocator::agent_coordinate(0);
        assert_eq!(c0.z.a, 1);
        assert_eq!(c0.x.c, 1); // scroll
        
        // Agent 8 should be at scroll 9
        let c8 = CoordinateAllocator::agent_coordinate(8);
        assert_eq!(c8.x.c, 9); // scroll
        
        // Agent 9 should wrap to section 2
        let c9 = CoordinateAllocator::agent_coordinate(9);
        assert_eq!(c9.x.c, 1); // scroll
        assert_eq!(c9.x.b, 2); // section
    }

    #[tokio::test]
    async fn test_client_not_connected() {
        let client = SqClient::new();
        assert!(!client.is_connected().await);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_coordinate_space() {
        // Verify we can address 9^9 = 387,420,489 scrolls
        let max = PhextCoordinate::boundary();
        let arr = max.to_array();
        assert!(arr.iter().all(|&x| x == 9));
        
        // Verify origin
        let min = PhextCoordinate::origin();
        let arr = min.to_array();
        assert!(arr.iter().all(|&x| x == 1));
    }

    #[test]
    fn test_pi_coordinate_properties() {
        // Pi coordinate encodes 3.14159265 across 9 dimensions
        let pi = PhextCoordinate::pi();
        let arr = pi.to_array();
        
        // Should spell out first 9 digits of pi
        assert_eq!(arr, [3, 1, 4, 1, 5, 9, 2, 6, 5]);
    }
}
