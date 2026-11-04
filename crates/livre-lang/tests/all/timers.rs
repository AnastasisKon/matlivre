//! timer objects (timer, start, stop, wait, timerfind, timerfindall), against MATLAB R2023b.
//! Timers fire in pause and wait, as they do in MATLAB while a script runs.

use livre_lang::{Interpreter, OutputKind};

fn shown(src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    let _ = it.run_source(src);
    it.take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Value { text, .. } => Some(text),
            OutputKind::Text(t) => Some(t),
            OutputKind::Warning(w) => Some(format!("Warning: {w}")),
            OutputKind::Error(e) => Some(format!("Error: {e}")),
            _ => None,
        })
        .collect()
}

const EMPTY_TIMER: &str = "t = \n   Timer Object: timer-1\n\n   Timer Settings\n      ExecutionMode: singleShot\n             Period: 1\n           BusyMode: drop\n            Running: off\n\n   Callbacks\n           TimerFcn: ''\n           ErrorFcn: ''\n           StartFcn: ''\n            StopFcn: ''";

#[test]
fn timers_fire_in_wait_and_pause() {
    assert_eq!(
        shown(concat!(
            "t = timer\nt.TimerFcn = @(~,~) disp('tick');\nt.StartDelay = 0.1;\n",
            "start(t); disp('started'); wait(t); disp('waited')\nt.Running\nt.TasksExecuted\ndelete(t)\n",
            "t2 = timer('Name', 'mine', 'TimerFcn', @(o,e) fprintf('%s %s\\n', o.Name, e.Type), 'Period', 0.05, 'ExecutionMode', 'fixedRate', ",
            "'TasksToExecute', 3, 'StopFcn', @(o,e) disp(['stop ' e.Type]), 'StartFcn', @(o,e) disp(['start ' e.Type]));\n",
            "start(t2); wait(t2);\nt2.TasksExecuted\nn = numel(timerfindall)\ntf = timerfind('Name', 'mine'); class(tf)\n",
            "t3 = timer('TimerFcn', 'disp(''str cb'')', 'StartDelay', 0.05);\nstart(t3); pause(0.3); disp('after pause')\n",
            "delete(timerfindall)\nnumel(timerfindall)\n",
            "t4 = timer('TimerFcn', @(~,~) disp('late'), 'StartDelay', 0.2);\nstart(t4); stop(t4); disp('stopped'); t4.Running\ndelete(t4)\n",
            "t6 = timer('TimerFcn', @(~,~) error('boom'));\nstart(t6); wait(t6); disp('after error')\ndelete(t6)\n",
        )),
        [
            EMPTY_TIMER,
            "started\n",
            "tick\n",
            "waited\n",
            "ans = 'off'",
            "ans = 1",
            "start StartFcn\n",
            "mine TimerFcn\n",
            "mine TimerFcn\n",
            "mine TimerFcn\n",
            "stop StopFcn\n",
            "ans = 3",
            "n = 1",
            "ans = 'timer'",
            "str cb\n",
            "after pause\n",
            "ans = 0",
            "stopped\n",
            "ans = 'off'",
            "Error while evaluating TimerFcn for timer 'timer-1' \n\nboom\n",
            "after error\n",
        ]
    );
}

#[test]
fn timer_arrays_and_properties() {
    assert_eq!(
        shown(concat!(
            "a = timer('TimerFcn', @(~,~) disp('x'), 'Period', 2.5, 'ExecutionMode', 'fixedSpacing', 'StopFcn', {@(o,e,k) disp(k), 7});\n",
            "b = timer('Name', 'bee');\nall = timerfindall\nsize(all)\nall(2).Name\na\nisvalid(a)\ndelete(a)\nisvalid(a)\n",
            "c = timer('TimerFcn', @(~,~) disp('y'), 'StartDelay', 5);\nstart(c)\n",
            "try, start(c), catch e, disp(e.message), end\n",
            "try, c.Period = -1; catch e, disp(e.message), end\n",
            "try, c.ExecutionMode = 'often'; catch e, disp(e.message), end\n",
            "try, c.Running = 'on'; catch e, disp(e.message), end\n",
            "try, c.BusyMode = 'x'; catch e, disp(e.message), end\n",
            "try, t5 = timer('Bogus', 1), catch e, disp(e.message), end\n",
            "stop(c)\ns = get(b); class(s), numel(fieldnames(s))\nset(b, 'Tag', 'hello'); b.Tag\nc.UserData = 42; get(c, 'UserData')\n",
            "c.Period = 0.0004;\nc.Period\n",
            "delete(timerfindall)\ny = timerfindall\ntry, start(c), catch e, disp(e.message), end\n",
        )),
        [
            "all = \nTimer Object Array\n\n   Index:  ExecutionMode:  Period:  TimerFcn:               Name:\n   1       fixedSpacing    2.5      @(~,~)disp('x')         timer-1\n   2       singleShot      1        ''                      bee",
            "ans = 1×2\n\n     1     2",
            "ans = 'bee'",
            "a = \n   Timer Object: timer-1\n\n   Timer Settings\n      ExecutionMode: fixedSpacing\n             Period: 2.5\n           BusyMode: drop\n            Running: off\n\n   Callbacks\n           TimerFcn: @(~,~)disp('x')\n           ErrorFcn: ''\n           StartFcn: ''\n            StopFcn: {{@(o,e,k)disp(k)} {[7]}}",
            "ans = logical\n   1",
            "ans = logical\n   0",
            "Cannot start timer because it is already running.\n",
            "Error setting property 'Period' of class 'timer'. Value must be positive.\n",
            "Error setting property 'ExecutionMode' of class 'timer'. 'often' is invalid. Value must be 'singleShot', 'fixedSpacing', 'fixedDelay', or 'fixedRate'.\n",
            "Unable to set the 'Running' property of class ''timer'' because it is read-only.\n",
            "Error setting property 'BusyMode' of class 'timer'. 'x' is invalid. Value must be 'drop', 'queue', or 'error'.\n",
            "The name 'Bogus' is not an accessible property for an instance of class 'timer'.\n",
            "ans = 'struct'",
            "ans = 18",
            "ans = 'hello'",
            "ans = 42",
            "Warning: Period property is limited to 1 millisecond precision. Sub-millisecond precision will be ignored.",
            "ans = 0",
            "y = []",
            "Invalid timer object. This object has been deleted and should be removed from your workspace using CLEAR.\n",
        ]
    );
}
