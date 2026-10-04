import { useEffect } from "react";
import { Outlet, useParams } from "react-router-dom";
import {
  useEngineActivateMutation,
  useEngineDeactivateMutation,
} from "@/store/api";

/** Keeps the profile's trackers running while any screen of the profile is open. */
export function ProfileRoute() {
  const { profileId = "" } = useParams();
  const [activate] = useEngineActivateMutation();
  const [deactivate] = useEngineDeactivateMutation();

  useEffect(() => {
    void activate({ profileId });
    return () => {
      void deactivate();
    };
  }, [profileId, activate, deactivate]);

  return <Outlet />;
}
