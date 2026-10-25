#[derive(Default)]
pub struct RestEnd {signal:bool,pub requested:bool}
impl RestEnd {
    pub fn request(&mut self,active:bool)->bool {
        if !active||self.requested{return false;}
        self.requested=true;true
    }
    pub fn observe(&mut self,active:bool,signal:bool)->bool {
        let began=active&&signal&&!self.signal&&!self.requested;
        self.signal=signal;
        if !active {self.requested=false;} else if began {self.requested=true;}
        began
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_only_ends_active_run_once() {
        let mut end=RestEnd::default();
        assert!(!end.request(false));assert!(!end.requested);
        assert!(end.request(true));assert!(!end.request(true));
        end.observe(true,false);assert!(end.requested);
        end.observe(false,false);assert!(!end.requested);
        assert!(end.request(true));
    }
    #[test]
    fn only_new_rest_during_run_ends_it_and_request_survives_loading() {
        let mut rest=RestEnd::default();
        assert!(!rest.observe(false,true));
        assert!(!rest.observe(true,true));
        assert!(!rest.observe(true,false));
        assert!(rest.observe(true,true));
        assert!(!rest.observe(true,false));assert!(rest.requested);
        assert!(!rest.observe(false,true));assert!(!rest.requested);
        assert!(!rest.observe(true,true));
    }
}
