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
    const Clock::time_point hiddenAt(std::chrono::milliseconds(250));
    const auto firstHidden = HiddenRenderClock(hiddenAt);
    assert(firstHidden.lastFrame == hiddenAt);
    assert(firstHidden.nextFrame == Clock::time_point::max());
    const Clock::time_point hiddenAgain(std::chrono::milliseconds(5000));
    const auto repeatedHidden = HiddenRenderClock(hiddenAgain);
    assert(repeatedHidden.lastFrame == hiddenAgain);
    assert(repeatedHidden.nextFrame == Clock::time_point::max());
  }

  {
    RenderPresentationState presentation;
    RenderState state;
    state.apply(Config(0, false));
    presentation.requestVisible(true);
    int primeCalls = 0;
    int showCalls = 0;
    const bool failedPrerequisite = TryPrimeAndShow(
        presentation, false, [&primeCalls]() {
          ++primeCalls;
          return true;
        },
        [&showCalls]() {
          ++showCalls;
          return true;
        });
    state.setVisible(presentation.actuallyVisible());
    assert(!failedPrerequisite);
    assert(presentation.requestedVisible());
    assert(!presentation.actuallyVisible());
    assert(state.targetFps(60) == 0);
    assert(primeCalls == 0);
    assert(showCalls == 0);

    const bool failedPrime = TryPrimeAndShow(
        presentation, true, [&primeCalls]() {
          ++primeCalls;
          return false;
        },
        [&showCalls]() {
          ++showCalls;
          return true;
        });
    assert(!failedPrime);
    assert(!presentation.actuallyVisible());
    assert(primeCalls == 1);
    assert(showCalls == 0);

    const bool recovered = TryPrimeAndShow(
        presentation, true, [&primeCalls]() {
          ++primeCalls;
          return true;
        },
        [&showCalls]() {
          ++showCalls;
          return true;
        });
    state.setVisible(presentation.actuallyVisible());
    assert(recovered);
    assert(presentation.requestedVisible());
    assert(presentation.actuallyVisible());
    assert(state.targetFps(60) == 30);
    assert(primeCalls == 2);
    assert(showCalls == 1);

    int concealCalls = 0;
    int resizeCalls = 0;
    const bool resized = TryResizeWhileConcealed(
        presentation, [&concealCalls]() { ++concealCalls; },
        [&presentation, &resizeCalls]() {
          ++resizeCalls;
          assert(!presentation.actuallyVisible());
          return true;
        });
    assert(resized);
    assert(concealCalls == 1);
    assert(resizeCalls == 1);
    assert(presentation.requestedVisible());
    assert(!presentation.actuallyVisible());

    presentation.requestVisible(false);
    state.setVisible(presentation.actuallyVisible());
    assert(!presentation.requestedVisible());
    assert(state.targetFps(60) == 0);
    presentation.requestVisible(true);
    state.setVisible(presentation.actuallyVisible());
    assert(presentation.requestedVisible());
    assert(!presentation.actuallyVisible());
    assert(state.targetFps(60) == 0);
  }

  {
    SurfacePrimeState surface;
    surface.markPrimed();
    assert(surface.reveal());
    assert(surface.canRender());

    surface.conceal();
    assert(!surface.primed());
    assert(!surface.reveal());
    assert(!surface.canRender());

    surface.markPrimed();
    assert(surface.reveal());
    assert(surface.canRender());
  }

  {
    RendererStatusState status;
    assert(status.value() == RendererAvailability::Unavailable);
    assert(!status.transition(RendererAvailability::Unavailable));
    assert(status.transition(RendererAvailability::Ready));
    assert(!status.transition(RendererAvailability::Ready));
    assert(status.transition(RendererAvailability::Unavailable));
  }

  {
    RendererRetryState retry;
    retry.request(100, true);
    assert(retry.due(100));
    retry.failed(100);
    assert(!retry.due(199));
    assert(retry.due(200));
    retry.failed(200);
    assert(retry.due(400));
    retry.failed(400);
    assert(retry.due(800));
    retry.failed(800);
    assert(!retry.pending());
    assert(retry.attempts() == 4);
    retry.request(850, false);
    assert(!retry.pending());
    assert(retry.attempts() == 4);
    retry.request(900, true);
    assert(retry.attempts() == 0);
    assert(retry.due(900));
    retry.succeeded();
    assert(!retry.pending());
    assert(retry.attempts() == 0);
    retry.request(1000, false);
    retry.cancel();
    assert(!retry.pending());
  }

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
    state.apply(Config(0, true, 119.0));
    assert(Near(state.configuredDiameter(), 120.0));
    state.apply(Config(0, true, 120.0));
    assert(Near(state.configuredDiameter(), 120.0));
    state.apply(Config(0, true, 220.0));
    assert(Near(state.configuredDiameter(), 220.0));
    state.apply(Config(0, true, 299.0));
    assert(Near(state.configuredDiameter(), 299.0));
    state.apply(Config(0, true, 300.0));
    assert(Near(state.configuredDiameter(), 300.0));
    state.apply(Config(0, true, 901.0));
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
