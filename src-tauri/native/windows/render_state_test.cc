#include "render_state.h"

#include <cassert>
#include <chrono>
#include <cmath>

namespace {

bool Near(double actual, double expected) {
  return std::abs(actual - expected) < 0.000001;
}

RenderConfig Config(uint32_t fps, bool visible, double size = 600.0,
                    uint32_t pendingCount = 0, uint8_t visualStyle = 0) {
  RenderConfig config{};
  config.fps = fps;
  config.visible = visible;
  config.size = size;
  config.pendingCount = pendingCount;
  config.visualStyle = visualStyle;
  return config;
}

}  // namespace

int main() {
  {
    using Clock = std::chrono::steady_clock;
    const Clock::time_point completed(std::chrono::milliseconds(10000));
    const Clock::time_point deadline = NextRenderDeadline(completed, 60);
    assert(deadline > completed);
    assert(std::chrono::duration_cast<std::chrono::milliseconds>(
               deadline - completed)
               .count() == 16);
    assert(NextRenderDeadline(completed, 0) == Clock::time_point::max());
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    assert(state.targetFps(60) == 30);
    state.setHover(true);
    assert(state.targetFps(60) == 60);
    assert(Near(state.visualDiameter(), state.configuredDiameter()));
    state.setVisible(false);
    assert(state.targetFps(60) == 0);
  }

  {
    RenderState state;
    state.apply(Config(30, true));
    assert(state.targetFps(144) == 30);
    state.setVisualState(RenderVisualState::WaitingForAck);
    assert(state.targetFps(144) == 30);
    state.apply(Config(60, true));
    assert(state.targetFps(30) == 60);
  }

  {
    RenderState state;
    state.apply(Config(0, true, 200.0));
    assert(Near(state.configuredDiameter(), 300.0));
    state.apply(Config(0, true, 1200.0));
    assert(Near(state.configuredDiameter(), 900.0));
    state.apply(Config(0, true, 540.0));
    assert(Near(state.configuredDiameter(), 540.0));
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.advance(-1.0);
    assert(Near(state.frame().animationTime, 0.0));
    state.advance(0.25);
    assert(Near(state.frame().animationTime, 0.1));
    state.setHoverProgress(1.0);
    state.advance(0.25);
    assert(Near(state.frame().animationTime, 0.34));
    assert(state.frame().animationTime > 0.0);
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.setHoverProgress(-1.0);
    assert(Near(state.frame().pullGain, 1.0));
    assert(Near(state.frame().rotationRate, 1.0));
    state.setHoverProgress(0.5);
    assert(Near(state.frame().pullGain, 1.35));
    assert(Near(state.frame().rotationRate, 1.7));
    state.setHoverProgress(2.0);
    assert(Near(state.frame().pullGain, 1.7));
    assert(Near(state.frame().rotationRate, 2.4));
    assert(Near(state.visualDiameter(), 600.0));
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.setVisualState(RenderVisualState::WaitingForAck);
    state.advance(0.37);
    assert(Near(state.frame().ingestProgress, 0.5));
    assert(Near(state.frame().ejectProgress, 0.0));
    assert(Near(state.frame().successJetProgress, 0.0));

    state.setVisualState(RenderVisualState::SwallowAndSuccessJet);
    state.advance(0.37);
    assert(Near(state.frame().ingestProgress, 1.0));
    state.advance(0.25);
    assert(Near(state.frame().successJetProgress, 0.5));
    state.advance(0.25);
    assert(state.visualState() == RenderVisualState::Idle);
    assert(Near(state.frame().ingestProgress, 0.0));
    assert(Near(state.frame().successJetProgress, 0.0));
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.setVisualState(RenderVisualState::SwallowAndEject);
    state.advance(0.74);
    assert(Near(state.frame().ingestProgress, 1.0));
    assert(Near(state.frame().ejectProgress, 0.0));
    state.advance(0.31);
    assert(Near(state.frame().ejectProgress, 0.5));
    state.advance(0.31);
    assert(state.visualState() == RenderVisualState::Idle);
  }

  {
    RenderState state;
    state.apply(Config(0, true));
    state.setVisualState(RenderVisualState::WaitingForAck);
    state.advance(2.0);
    assert(Near(state.frame().ingestProgress, 1.0));
    state.setVisualState(RenderVisualState::SwallowAndSuccessJet);
    assert(state.visualState() == RenderVisualState::SwallowAndSuccessJet);
    state.advance(0.25);
    assert(Near(state.frame().successJetProgress, 0.5));
  }

  {
    RenderState state;
    state.apply(Config(0, true, 600.0, 7, 0));
    assert(state.frame().pendingCount == 7);
    assert(state.frame().shaderStyle == 1);
    state.apply(Config(0, true, 600.0, 2, 1));
    assert(state.frame().pendingCount == 2);
    assert(state.frame().shaderStyle == 0);
  }

  return 0;
}
